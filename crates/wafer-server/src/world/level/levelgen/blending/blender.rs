use std::sync::{Arc, OnceLock};

use wafer_core::ChunkPos;
use wafer_core::fastutil::Long2ObjectOpenHashMap;
use wafer_core::random::XoroshiroRandomSource;
use wafer_core::mth;

use super::blending_data::{BlendingData, Direction8, NO_VALUE};
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::AlgorithmRandom;
use crate::world::level::levelgen::density_function::DensityVolume;
use crate::world::level::levelgen::synth::{NoiseStack, NormalNoise};

const HEIGHT_BLENDING_RANGE_CELLS: i32 = (7 << 2) - 1;
pub const HEIGHT_BLENDING_RANGE_CHUNKS: i32 = (HEIGHT_BLENDING_RANGE_CELLS + 3) >> 2;
const DENSITY_BLENDING_RANGE_CHUNKS: i32 = 5 >> 2;

fn shift_noise() -> &'static NoiseStack {
    static SHIFT_NOISE: OnceLock<NoiseStack> = OnceLock::new();
    SHIFT_NOISE.get_or_init(|| NormalNoise::create_parity(-3, &[1.0, 1.0, 1.0, 0.0]).create(&mut AlgorithmRandom::Xoroshiro(XoroshiroRandomSource::new(42))))
}

pub struct Blender {
    height_and_biome_blending_data: Long2ObjectOpenHashMap<Arc<BlendingData>>,
    density_blending_data: Long2ObjectOpenHashMap<Arc<BlendingData>>,
    carving_filter: Option<CarvingFilter>,
}

#[derive(Clone, Copy)]
pub struct BlendingOutput {
    pub alpha: f32,
    pub blending_offset: f32,
}

impl Blender {
    pub fn empty() -> Blender {
        Blender { height_and_biome_blending_data: Long2ObjectOpenHashMap::new(), density_blending_data: Long2ObjectOpenHashMap::new(), carving_filter: None }
    }

    pub fn of(center: ChunkPos, mut blending_data: impl FnMut(i32, i32) -> Option<Arc<BlendingData>>, carving_filter: Option<CarvingFilter>) -> Blender {
        let mut height_and_biome_data = Long2ObjectOpenHashMap::new();
        let mut density_data = Long2ObjectOpenHashMap::new();
        let max_dist_sq = (HEIGHT_BLENDING_RANGE_CHUNKS + 1) * (HEIGHT_BLENDING_RANGE_CHUNKS + 1);
        for dx in -HEIGHT_BLENDING_RANGE_CHUNKS..=HEIGHT_BLENDING_RANGE_CHUNKS {
            for dz in -HEIGHT_BLENDING_RANGE_CHUNKS..=HEIGHT_BLENDING_RANGE_CHUNKS {
                if dx * dx + dz * dz <= max_dist_sq {
                    let (chunk_x, chunk_z) = (center.x + dx, center.z + dz);
                    if let Some(data) = blending_data(chunk_x, chunk_z) {
                        let key = ChunkPos::new(chunk_x, chunk_z).pack();
                        if (-DENSITY_BLENDING_RANGE_CHUNKS..=DENSITY_BLENDING_RANGE_CHUNKS).contains(&dx) && (-DENSITY_BLENDING_RANGE_CHUNKS..=DENSITY_BLENDING_RANGE_CHUNKS).contains(&dz) {
                            density_data.insert(key, data.clone());
                        }
                        height_and_biome_data.insert(key, data);
                    }
                }
            }
        }
        Blender { height_and_biome_blending_data: height_and_biome_data, density_blending_data: density_data, carving_filter }
    }

    pub fn is_empty(&self) -> bool {
        self.height_and_biome_blending_data.is_empty() && self.density_blending_data.is_empty() && self.carving_filter.is_none()
    }

    pub fn blend_offset_and_factor_volume(self: &Arc<Blender>, volume: &DensityVolume) -> OutputBuffer {
        let min_cell_x = volume.min_x >> 2;
        let min_cell_z = volume.min_z >> 2;
        let max_cell_x = (volume.block_x(volume.size_x - 1) >> 2) + 1;
        let max_cell_z = (volume.block_z(volume.size_z - 1) >> 2) + 1;
        let cell_count_x = max_cell_x - min_cell_x + 1;
        let cell_count_z = max_cell_z - min_cell_z + 1;
        let mut alphas = vec![0.0f32; (cell_count_x * cell_count_z) as usize];
        let mut offsets = vec![0.0f32; (cell_count_x * cell_count_z) as usize];
        for cell_z in 0..cell_count_z {
            for cell_x in 0..cell_count_x {
                let index = (cell_x + cell_z * cell_count_x) as usize;
                let output = self.blend_offset_and_factor((min_cell_x + cell_x) << 2, (min_cell_z + cell_z) << 2);
                alphas[index] = output.alpha;
                offsets[index] = output.blending_offset;
            }
        }
        OutputBuffer { blender: self.clone(), min_cell_x, min_cell_z, cell_count_x, cell_count_z, alphas, offsets }
    }

    pub fn blend_offset_and_factor(&self, block_x: i32, block_z: i32) -> BlendingOutput {
        if self.height_and_biome_blending_data.is_empty() && self.density_blending_data.is_empty() && self.carving_filter.is_none() {
            return BlendingOutput { alpha: 1.0, blending_offset: 0.0 };
        }
        let cell_x = block_x >> 2;
        let cell_z = block_z >> 2;
        let fixed_height = self.get_blending_data_value(cell_x, 0, cell_z, BlendingData::get_height);
        if fixed_height != NO_VALUE {
            return BlendingOutput { alpha: 0.0, blending_offset: height_to_offset(fixed_height as f64) as f32 };
        }
        let mut total_weight = 0.0f64;
        let mut weighted_heights = 0.0f64;
        let mut closest_distance = f64::INFINITY;
        for (chunk_pos, blending_data) in self.height_and_biome_blending_data.iter() {
            let chunk_pos = ChunkPos::unpack(chunk_pos);
            blending_data.iterate_heights(chunk_pos.x << 2, chunk_pos.z << 2, |test_cell_x, test_cell_z, height| {
                let distance = length2((cell_x - test_cell_x) as f64, (cell_z - test_cell_z) as f64);
                if !(distance > HEIGHT_BLENDING_RANGE_CELLS as f64) {
                    if distance < closest_distance {
                        closest_distance = distance;
                    }
                    let weight = 1.0 / (distance * distance * distance * distance);
                    weighted_heights += height as f64 * weight;
                    total_weight += weight;
                }
            });
        }
        if closest_distance == f64::INFINITY {
            return BlendingOutput { alpha: 1.0, blending_offset: 0.0 };
        }
        let average_height = weighted_heights / total_weight;
        let mut alpha = (closest_distance / (HEIGHT_BLENDING_RANGE_CELLS + 1) as f64).clamp(0.0, 1.0);
        alpha = 3.0 * alpha * alpha - 2.0 * alpha * alpha * alpha;
        BlendingOutput { alpha: alpha as f32, blending_offset: height_to_offset(average_height) as f32 }
    }

    pub fn blend_density(&self, block_x: i32, block_y: i32, block_z: i32, noise_value: f32) -> f32 {
        if self.is_empty() {
            return noise_value;
        }
        let cell_x = block_x >> 2;
        let cell_y = block_y / 8;
        let cell_z = block_z >> 2;
        let fixed_density = self.get_blending_data_value(cell_x, cell_y, cell_z, BlendingData::get_density);
        if fixed_density != NO_VALUE {
            return fixed_density;
        }
        let mut total_weight = 0.0f64;
        let mut weighted_heights = 0.0f64;
        let mut closest_distance = f64::INFINITY;
        for (chunk_pos, blending_data) in self.density_blending_data.iter() {
            let chunk_pos = ChunkPos::unpack(chunk_pos);
            blending_data.iterate_densities(chunk_pos.x << 2, chunk_pos.z << 2, cell_y - 1, cell_y + 1, |test_cell_x, test_cell_y, test_cell_z, density| {
                let distance = length3((cell_x - test_cell_x) as f64, ((cell_y - test_cell_y) * 2) as f64, (cell_z - test_cell_z) as f64);
                if !(distance > 2.0) {
                    if distance < closest_distance {
                        closest_distance = distance;
                    }
                    let weight = 1.0 / (distance * distance * distance * distance);
                    weighted_heights += density as f64 * weight;
                    total_weight += weight;
                }
            });
        }
        if closest_distance == f64::INFINITY {
            return noise_value;
        }
        let average_density = weighted_heights / total_weight;
        let alpha = (closest_distance / 3.0).clamp(0.0, 1.0);
        (average_density + alpha * (noise_value as f64 - average_density)) as f32
    }

    fn get_blending_data_value(&self, cell_x: i32, cell_y: i32, cell_z: i32, cell_value_getter: fn(&BlendingData, i32, i32, i32) -> f32) -> f32 {
        let chunk_x = cell_x >> 2;
        let chunk_z = cell_z >> 2;
        let min_x = (cell_x & 3) == 0;
        let min_z = (cell_z & 3) == 0;
        let mut value = self.get_blending_data_value_in(cell_value_getter, chunk_x, chunk_z, cell_x, cell_y, cell_z);
        if value == NO_VALUE {
            if min_x && min_z {
                value = self.get_blending_data_value_in(cell_value_getter, chunk_x - 1, chunk_z - 1, cell_x, cell_y, cell_z);
            }
            if value == NO_VALUE {
                if min_x {
                    value = self.get_blending_data_value_in(cell_value_getter, chunk_x - 1, chunk_z, cell_x, cell_y, cell_z);
                }
                if value == NO_VALUE && min_z {
                    value = self.get_blending_data_value_in(cell_value_getter, chunk_x, chunk_z - 1, cell_x, cell_y, cell_z);
                }
            }
        }
        value
    }

    fn get_blending_data_value_in(&self, cell_value_getter: fn(&BlendingData, i32, i32, i32) -> f32, chunk_x: i32, chunk_z: i32, cell_x: i32, cell_y: i32, cell_z: i32) -> f32 {
        match self.height_and_biome_blending_data.get(ChunkPos::new(chunk_x, chunk_z).pack()) {
            Some(blending_data) => cell_value_getter(blending_data, cell_x - (chunk_x << 2), cell_y, cell_z - (chunk_z << 2)),
            None => NO_VALUE,
        }
    }

    pub fn blend_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Option<Biome> {
        if self.is_empty() {
            return None;
        }
        let mut closest_distance = f32::INFINITY;
        let mut closest_biome = None;
        for (chunk_pos, blending_data) in self.height_and_biome_blending_data.iter() {
            let chunk_pos = ChunkPos::unpack(chunk_pos);
            blending_data.iterate_biomes(chunk_pos.x << 2, quart_y, chunk_pos.z << 2, |test_cell_x, test_cell_z, biome| {
                let (dx, dz) = ((quart_x - test_cell_x) as f32, (quart_z - test_cell_z) as f32);
                let distance = (dx * dx + dz * dz).sqrt();
                if !(distance > HEIGHT_BLENDING_RANGE_CELLS as f32) && distance < closest_distance {
                    closest_biome = Some(biome);
                    closest_distance = distance;
                }
            });
        }
        if closest_distance == f32::INFINITY {
            return None;
        }
        let shift_noise = shift_noise().get(quart_x as f64, 0.0, quart_z as f64) * 12.0;
        let alpha = ((closest_distance + shift_noise) / (HEIGHT_BLENDING_RANGE_CELLS + 1) as f32).clamp(0.0, 1.0);
        if alpha > 0.5 { None } else { closest_biome }
    }

    pub fn get_carving_filter(&self) -> Option<&CarvingFilter> {
        self.carving_filter.as_ref()
    }
}

fn height_to_offset(height: f64) -> f64 {
    let target_y = height + 0.5;
    let target_y_mod = mth::positive_modulo_d(target_y, 8.0);
    1.0 * (32.0 * (target_y - 128.0) - 3.0 * (target_y - 120.0) * target_y_mod + 3.0 * target_y_mod * target_y_mod) / (128.0 * (32.0 - 3.0 * target_y_mod))
}

fn length2(x: f64, z: f64) -> f64 {
    (x * x + z * z).sqrt()
}

fn length3(x: f64, y: f64, z: f64) -> f64 {
    (x * x + y * y + z * z).sqrt()
}

pub struct OutputBuffer {
    blender: Arc<Blender>,
    min_cell_x: i32,
    min_cell_z: i32,
    cell_count_x: i32,
    cell_count_z: i32,
    alphas: Vec<f32>,
    offsets: Vec<f32>,
}

impl OutputBuffer {
    pub fn blender(&self) -> &Blender {
        &self.blender
    }

    pub fn get_alpha(&self, block_x: i32, block_z: i32) -> f32 {
        match self.get_index(block_x, block_z) {
            Some(index) => self.alphas[index],
            None => self.blender.blend_offset_and_factor(block_x, block_z).alpha,
        }
    }

    pub fn get_offset(&self, block_x: i32, block_z: i32) -> f32 {
        match self.get_index(block_x, block_z) {
            Some(index) => self.offsets[index],
            None => self.blender.blend_offset_and_factor(block_x, block_z).blending_offset,
        }
    }

    fn get_index(&self, block_x: i32, block_z: i32) -> Option<usize> {
        let relative_x = (block_x >> 2) - self.min_cell_x;
        let relative_z = (block_z >> 2) - self.min_cell_z;
        (relative_x >= 0 && relative_z >= 0 && relative_x < self.cell_count_x && relative_z < self.cell_count_z).then_some((relative_x + relative_z * self.cell_count_x) as usize)
    }
}

pub struct CarvingFilter {
    distance_getters: Vec<(f64, f64, f64, f64)>,
}

impl CarvingFilter {
    pub fn around_old_chunks(center_blending_data: Option<&BlendingData>, old_sides_blending_data: &[(Direction8, Arc<BlendingData>)]) -> Option<CarvingFilter> {
        if center_blending_data.is_none() && old_sides_blending_data.is_empty() {
            return None;
        }
        let mut distance_getters = Vec::new();
        if let Some(center) = center_blending_data {
            distance_getters.push(Self::offset_old_chunk_distance_getter(None, center));
        }
        for (side, blending_data) in old_sides_blending_data {
            distance_getters.push(Self::offset_old_chunk_distance_getter(Some(*side), blending_data));
        }
        Some(CarvingFilter { distance_getters })
    }

    fn offset_old_chunk_distance_getter(offset: Option<Direction8>, blending_data: &BlendingData) -> (f64, f64, f64, f64) {
        let (offset_x, offset_z) = offset.map_or((0.0, 0.0), |offset| ((offset.step_x() * 16) as f64, (offset.step_z() * 16) as f64));
        let old_chunk_y_radius = blending_data.area_height() as f64 / 2.0;
        let old_chunk_center_y = blending_data.area_min_y() as f64 + old_chunk_y_radius;
        (offset_x, offset_z, old_chunk_y_radius, old_chunk_center_y)
    }

    pub fn test(&self, x: i32, y: i32, z: i32) -> bool {
        let noise = shift_noise();
        let shifted_x = x as f64 + 0.5 + (noise.get(x as f64, y as f64, z as f64) * 4.0) as f64;
        let shifted_y = y as f64 + 0.5 + (noise.get(y as f64, z as f64, x as f64) * 4.0) as f64;
        let shifted_z = z as f64 + 0.5 + (noise.get(z as f64, x as f64, y as f64) * 4.0) as f64;
        let mut closest = f64::INFINITY;
        for &(offset_x, offset_z, radius_y, center_y) in &self.distance_getters {
            let distance = distance_to_cube(shifted_x - 8.0 - offset_x, shifted_y - center_y, shifted_z - 8.0 - offset_z, 8.0, radius_y, 8.0);
            if distance < closest {
                closest = distance;
            }
        }
        closest < 4.0
    }
}

fn distance_to_cube(x: f64, y: f64, z: f64, radius_x: f64, radius_y: f64, radius_z: f64) -> f64 {
    let delta_x = x.abs() - radius_x;
    let delta_y = y.abs() - radius_y;
    let delta_z = z.abs() - radius_z;
    length3(delta_x.max(0.0), delta_y.max(0.0), delta_z.max(0.0))
}
