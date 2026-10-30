use wafer_core::BlockPos;
use wafer_data::{Block, BlockState, block_tags, blocks};
use wafer_protocol::nbt::Nbt;

use crate::world::level::chunk::{Biome, HeightmapType, LevelChunk};

const QUARTS_PER_SECTION: i32 = 4;
pub const CELL_HORIZONTAL_MAX_INDEX_INSIDE: i32 = QUARTS_PER_SECTION - 1;
pub const CELL_HORIZONTAL_MAX_INDEX_OUTSIDE: i32 = QUARTS_PER_SECTION;
const CELL_COLUMN_INSIDE_COUNT: i32 = 2 * CELL_HORIZONTAL_MAX_INDEX_INSIDE + 1;
const CELL_COLUMN_OUTSIDE_COUNT: i32 = 2 * CELL_HORIZONTAL_MAX_INDEX_OUTSIDE + 1;
pub const CELL_COLUMN_COUNT: usize = (CELL_COLUMN_INSIDE_COUNT + CELL_COLUMN_OUTSIDE_COUNT) as usize;
pub const NO_VALUE: f32 = f32::MAX;
const SURFACE_BLOCKS: [Block; 11] = [
    blocks::PODZOL,
    blocks::GRAVEL,
    blocks::GRASS_BLOCK,
    blocks::STONE,
    blocks::COARSE_DIRT,
    blocks::SAND,
    blocks::RED_SAND,
    blocks::MYCELIUM,
    blocks::SNOW_BLOCK,
    blocks::TERRACOTTA,
    blocks::DIRT,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction8 {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Direction8 {
    pub const VALUES: [Direction8; 8] =
        [Direction8::North, Direction8::NorthEast, Direction8::East, Direction8::SouthEast, Direction8::South, Direction8::SouthWest, Direction8::West, Direction8::NorthWest];

    pub fn step_x(self) -> i32 {
        match self {
            Direction8::NorthEast | Direction8::East | Direction8::SouthEast => 1,
            Direction8::SouthWest | Direction8::West | Direction8::NorthWest => -1,
            Direction8::North | Direction8::South => 0,
        }
    }

    pub fn step_z(self) -> i32 {
        match self {
            Direction8::North | Direction8::NorthEast | Direction8::NorthWest => -1,
            Direction8::SouthEast | Direction8::South | Direction8::SouthWest => 1,
            Direction8::East | Direction8::West => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlendingData {
    min_y: i32,
    height: i32,
    has_calculated_data: bool,
    heights: [f32; CELL_COLUMN_COUNT],
    biomes: Vec<Option<Vec<Biome>>>,
    densities: Vec<Option<Vec<f32>>>,
}

impl BlendingData {
    fn new(min_section: i32, max_section: i32, heights: Option<[f32; CELL_COLUMN_COUNT]>) -> BlendingData {
        let min_y = min_section << 4;
        BlendingData {
            min_y,
            height: (max_section << 4) - min_y,
            has_calculated_data: false,
            heights: heights.unwrap_or([NO_VALUE; CELL_COLUMN_COUNT]),
            biomes: vec![None; CELL_COLUMN_COUNT],
            densities: vec![None; CELL_COLUMN_COUNT],
        }
    }

    pub fn unpack(tag: &Nbt) -> Option<BlendingData> {
        let min_section = tag.get("min_section").and_then(Nbt::as_i64)? as i32;
        let max_section = tag.get("max_section").and_then(Nbt::as_i64)? as i32;
        let heights: Option<Vec<f32>> = match tag.get("heights") {
            Some(Nbt::List(items)) => items.iter().map(|item| item.as_f64().map(|value| value as f32)).collect(),
            Some(Nbt::IntArray(values)) => Some(values.iter().map(|&value| value as f32).collect()),
            Some(Nbt::LongArray(values)) => Some(values.iter().map(|&value| value as f32).collect()),
            Some(Nbt::ByteArray(values)) => Some(values.iter().map(|&value| value as f32).collect()),
            _ => None,
        };
        let heights = match heights {
            Some(heights) => Some(<[f32; CELL_COLUMN_COUNT]>::try_from(heights).ok()?),
            None => None,
        };
        Some(BlendingData::new(min_section, max_section, heights))
    }

    pub fn pack(&self) -> Nbt {
        let mut tag = vec![("min_section".to_string(), Nbt::Int(self.min_section_y())), ("max_section".to_string(), Nbt::Int(self.max_section_y() + 1))];
        if self.heights.iter().any(|&height| height != NO_VALUE) {
            tag.push(("heights".to_string(), Nbt::List(self.heights.iter().map(|&height| Nbt::Float(height)).collect())));
        }
        Nbt::Compound(tag.into())
    }

    pub fn area_min_y(&self) -> i32 {
        self.min_y
    }

    pub fn area_max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn area_height(&self) -> i32 {
        self.height
    }

    fn min_section_y(&self) -> i32 {
        self.min_y >> 4
    }

    fn max_section_y(&self) -> i32 {
        self.area_max_y() >> 4
    }

    fn sections_count(&self) -> i32 {
        self.max_section_y() - self.min_section_y() + 1
    }

    pub fn has_calculated_data(&self) -> bool {
        self.has_calculated_data
    }

    pub fn calculate_data(&mut self, chunk: &LevelChunk, new_sides: &[Direction8]) {
        if self.has_calculated_data {
            return;
        }
        let has = |direction: Direction8| new_sides.contains(&direction);
        if has(Direction8::North) || has(Direction8::West) || has(Direction8::NorthWest) {
            self.add_values_for_column(get_inside_index(0, 0), chunk, 0, 0);
        }
        if has(Direction8::North) {
            for i in 1..QUARTS_PER_SECTION {
                self.add_values_for_column(get_inside_index(i, 0), chunk, 4 * i, 0);
            }
        }
        if has(Direction8::West) {
            for i in 1..QUARTS_PER_SECTION {
                self.add_values_for_column(get_inside_index(0, i), chunk, 0, 4 * i);
            }
        }
        if has(Direction8::East) {
            for i in 1..QUARTS_PER_SECTION {
                self.add_values_for_column(get_outside_index(CELL_HORIZONTAL_MAX_INDEX_OUTSIDE, i), chunk, 15, 4 * i);
            }
        }
        if has(Direction8::South) {
            for i in 0..QUARTS_PER_SECTION {
                self.add_values_for_column(get_outside_index(i, CELL_HORIZONTAL_MAX_INDEX_OUTSIDE), chunk, 4 * i, 15);
            }
        }
        if has(Direction8::East) && has(Direction8::NorthEast) {
            self.add_values_for_column(get_outside_index(CELL_HORIZONTAL_MAX_INDEX_OUTSIDE, 0), chunk, 15, 0);
        }
        if has(Direction8::East) && has(Direction8::South) && has(Direction8::SouthEast) {
            self.add_values_for_column(get_outside_index(CELL_HORIZONTAL_MAX_INDEX_OUTSIDE, CELL_HORIZONTAL_MAX_INDEX_OUTSIDE), chunk, 15, 15);
        }
        self.has_calculated_data = true;
    }

    fn add_values_for_column(&mut self, index: usize, chunk: &LevelChunk, block_x: i32, block_z: i32) {
        if self.heights[index] == NO_VALUE {
            self.heights[index] = self.get_height_at_xz(chunk, block_x, block_z) as f32;
        }
        self.densities[index] = Some(self.get_density_column(chunk, block_x, block_z, self.heights[index].floor() as i32));
        self.biomes[index] = Some(self.get_biome_column(chunk, block_x, block_z));
    }

    fn get_height_at_xz(&self, chunk: &LevelChunk, block_x: i32, block_z: i32) -> i32 {
        let height = match chunk.primed_worldgen_height(HeightmapType::WorldSurfaceWg, block_x, block_z) {
            Some(height) => height.min(self.area_max_y()),
            None => self.area_max_y(),
        };
        let min_y = self.area_min_y();
        let mut y = height;
        while y > min_y {
            if SURFACE_BLOCKS.contains(&chunk.block_state(BlockPos::new(block_x, y, block_z)).block()) {
                return y;
            }
            y -= 1;
        }
        min_y
    }

    fn get_density_column(&self, chunk: &LevelChunk, x: i32, z: i32, height: i32) -> Vec<f32> {
        let mut densities = vec![-1.0f32; self.cell_count_per_column()];
        let mut y = self.area_max_y() + 1;
        let mut last7 = read7(chunk, x, &mut y, z);
        for cell_index in (0..densities.len() - 1).rev() {
            let one = read1(chunk, x, &mut y, z);
            let current7 = read7(chunk, x, &mut y, z);
            densities[cell_index] = (last7 + one + current7) / 15.0;
            last7 = current7;
        }
        let highest_cell_with_surface_index = self.get_cell_y_index(height.div_euclid(8));
        if highest_cell_with_surface_index >= 0 && (highest_cell_with_surface_index as usize) < densities.len() - 1 {
            let in_cell_index = (height as f32 + 0.5) % 8.0 / 8.0;
            let amplitude_above_to_make_surface_be_at_height = (1.0 - in_cell_index) / in_cell_index;
            let max = amplitude_above_to_make_surface_be_at_height.max(1.0) * 0.25;
            densities[highest_cell_with_surface_index as usize + 1] = -amplitude_above_to_make_surface_be_at_height / max;
            densities[highest_cell_with_surface_index as usize] = 1.0 / max;
        }
        densities
    }

    fn get_biome_column(&self, chunk: &LevelChunk, block_x: i32, block_z: i32) -> Vec<Biome> {
        let min_quart_y = self.area_min_y() >> 2;
        (0..self.quart_count_per_column()).map(|quart_index| chunk.noise_biome(block_x >> 2, quart_index as i32 + min_quart_y, block_z >> 2)).collect()
    }

    pub fn get_height(&self, cell_x: i32, _cell_y: i32, cell_z: i32) -> f32 {
        if cell_x == CELL_HORIZONTAL_MAX_INDEX_OUTSIDE || cell_z == CELL_HORIZONTAL_MAX_INDEX_OUTSIDE {
            self.heights[get_outside_index(cell_x, cell_z)]
        } else if cell_x != 0 && cell_z != 0 {
            NO_VALUE
        } else {
            self.heights[get_inside_index(cell_x, cell_z)]
        }
    }

    fn get_density_in(&self, density_column: Option<&Vec<f32>>, cell_y: i32) -> f32 {
        let Some(density_column) = density_column else { return NO_VALUE };
        let y_index = self.get_cell_y_index(cell_y);
        if y_index >= 0 && (y_index as usize) < density_column.len() { density_column[y_index as usize] * 0.1 } else { NO_VALUE }
    }

    pub fn get_density(&self, cell_x: i32, cell_y: i32, cell_z: i32) -> f32 {
        if cell_y == self.get_min_y() {
            0.1
        } else if cell_x == CELL_HORIZONTAL_MAX_INDEX_OUTSIDE || cell_z == CELL_HORIZONTAL_MAX_INDEX_OUTSIDE {
            self.get_density_in(self.densities[get_outside_index(cell_x, cell_z)].as_ref(), cell_y)
        } else if cell_x != 0 && cell_z != 0 {
            NO_VALUE
        } else {
            self.get_density_in(self.densities[get_inside_index(cell_x, cell_z)].as_ref(), cell_y)
        }
    }

    pub fn iterate_biomes(&self, min_cell_x: i32, quart_y: i32, min_cell_z: i32, mut biome_consumer: impl FnMut(i32, i32, Biome)) {
        let (min_quart_y, max_quart_y) = (self.area_min_y() >> 2, self.area_max_y() >> 2);
        if quart_y < min_quart_y || quart_y > max_quart_y {
            return;
        }
        let quart_index = (quart_y - min_quart_y) as usize;
        for (i, biome_cell) in self.biomes.iter().enumerate() {
            if let Some(biome_cell) = biome_cell {
                biome_consumer(min_cell_x + get_x(i as i32), min_cell_z + get_z(i as i32), biome_cell[quart_index]);
            }
        }
    }

    pub fn iterate_heights(&self, min_cell_x: i32, min_cell_z: i32, mut height_consumer: impl FnMut(i32, i32, f32)) {
        for (i, &value) in self.heights.iter().enumerate() {
            if value != NO_VALUE {
                height_consumer(min_cell_x + get_x(i as i32), min_cell_z + get_z(i as i32), value);
            }
        }
    }

    pub fn iterate_densities(&self, min_cell_x: i32, min_cell_z: i32, from_cell_y: i32, to_cell_y: i32, mut density_consumer: impl FnMut(i32, i32, i32, f32)) {
        let min_cell_y = self.get_column_min_y();
        let min_y_index = (from_cell_y - min_cell_y).max(0);
        let max_y_index = (self.cell_count_per_column() as i32).min(to_cell_y - min_cell_y);
        for (i, density_column) in self.densities.iter().enumerate() {
            if let Some(density_column) = density_column {
                let (test_cell_x, test_cell_z) = (min_cell_x + get_x(i as i32), min_cell_z + get_z(i as i32));
                for y_index in min_y_index..max_y_index {
                    density_consumer(test_cell_x, y_index + min_cell_y, test_cell_z, density_column[y_index as usize] * 0.1);
                }
            }
        }
    }

    fn cell_count_per_column(&self) -> usize {
        (self.sections_count() * 2) as usize
    }

    fn quart_count_per_column(&self) -> usize {
        (self.sections_count() * 4) as usize
    }

    fn get_column_min_y(&self) -> i32 {
        self.get_min_y() + 1
    }

    fn get_min_y(&self) -> i32 {
        self.min_section_y() * 2
    }

    fn get_cell_y_index(&self, cell_y: i32) -> i32 {
        cell_y - self.get_column_min_y()
    }
}

fn read1(chunk: &LevelChunk, x: i32, y: &mut i32, z: i32) -> f32 {
    *y -= 1;
    if is_ground(chunk.block_state(BlockPos::new(x, *y, z))) { 1.0 } else { -1.0 }
}

fn read7(chunk: &LevelChunk, x: i32, y: &mut i32, z: i32) -> f32 {
    let mut sum = 0.0f32;
    for _ in 0..7 {
        sum += read1(chunk, x, y, z);
    }
    sum
}

fn is_ground(state: BlockState) -> bool {
    if state.is_air() || state.is(&block_tags::LEAVES) || state.is(&block_tags::LOGS) {
        return false;
    }
    if state.is_block(blocks::BROWN_MUSHROOM_BLOCK) || state.is_block(blocks::RED_MUSHROOM_BLOCK) {
        return false;
    }
    if state.is_block(blocks::SCAFFOLDING) || state.is_block(blocks::POWDER_SNOW) {
        return false;
    }
    !state.collision_voxel_shape().is_empty()
}

fn get_inside_index(x: i32, z: i32) -> usize {
    (CELL_HORIZONTAL_MAX_INDEX_INSIDE - x + z) as usize
}

fn get_outside_index(x: i32, z: i32) -> usize {
    (CELL_COLUMN_INSIDE_COUNT + x + CELL_HORIZONTAL_MAX_INDEX_OUTSIDE - z) as usize
}

fn get_x(index: i32) -> i32 {
    if index < CELL_COLUMN_INSIDE_COUNT {
        return zero_if_negative(CELL_HORIZONTAL_MAX_INDEX_INSIDE - index);
    }
    let offset_index = index - CELL_COLUMN_INSIDE_COUNT;
    CELL_HORIZONTAL_MAX_INDEX_OUTSIDE - zero_if_negative(CELL_HORIZONTAL_MAX_INDEX_OUTSIDE - offset_index)
}

fn get_z(index: i32) -> i32 {
    if index < CELL_COLUMN_INSIDE_COUNT {
        return zero_if_negative(index - CELL_HORIZONTAL_MAX_INDEX_INSIDE);
    }
    let offset_index = index - CELL_COLUMN_INSIDE_COUNT;
    CELL_HORIZONTAL_MAX_INDEX_OUTSIDE - zero_if_negative(offset_index - CELL_HORIZONTAL_MAX_INDEX_OUTSIDE)
}

fn zero_if_negative(value: i32) -> i32 {
    value & !(value >> 31)
}
