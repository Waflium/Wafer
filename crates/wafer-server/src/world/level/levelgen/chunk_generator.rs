use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use wafer_core::random::RandomSource;
use wafer_core::ChunkPos;
use wafer_data::{BlockState, DimensionType, block_tags, blocks};

use super::aquifer::{Aquifer, AquiferSamplers, FluidPicker, NoiseBasedAquifer};
use super::carver::BIOME_CARVERS;
use super::density_function::{Df, DensityVolume, Sampler, SamplerContext};
use super::material::WorldGenerationContext;
use super::worldgen_random::WorldgenRandom;
use crate::world::level::biome::biome_generation_settings;
use crate::world::level::biome::feature_sorter::{self, StepFeatureData};
use super::{NoiseGeneratorSettings, RandomState, WORLDGEN, quart_pos};
use crate::world::level::biome::climate::{self, Parameter};
use crate::world::level::biome::biome_source::BiomeSource;
use crate::world::level::biome::the_end_biome_source::TheEndBiomeSource;
use crate::world::level::biome::{self, MultiNoiseBiomeSource};
use super::beardifier::Beardifier;
use super::below_zero_retrogen::{self, BelowZeroRetrogen};
use super::blending::blender::{Blender, CarvingFilter};
use crate::world::level::chunk::carving_mask::CarvingMask;
use crate::world::level::chunk::proto_chunk::{ChunkStatus, ProtoChunk};
use crate::world::level::chunk::{Biome, HeightmapType, LevelChunk, LevelChunkSection};
use crate::world::level::noise_column::NoiseColumn;

const MAX_CACHED_BIOMES: usize = 16384;

const CARVER_RANGE: i32 = 8;
const PROTECTED_BLOCKS_ON_TOP: i32 = 7;

pub struct ChunkBiomes {
    min_quart_y: i32,
    quarts: Vec<Biome>,
}

impl ChunkBiomes {
    pub fn from_sections(min_y: i32, sections: &[LevelChunkSection]) -> ChunkBiomes {
        let mut quarts = vec![Biome(0); sections.len() * 64];
        for (i, section) in sections.iter().enumerate() {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        quarts[(((i * 4 + y) * 4 + x) * 4 + z) as usize] = section.biomes.get(x, y, z);
                    }
                }
            }
        }
        ChunkBiomes { min_quart_y: quart_pos::from_block(min_y), quarts }
    }

    pub fn get(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let max = self.min_quart_y + (self.quarts.len() / 16) as i32 - 1;
        let y = quart_y.clamp(self.min_quart_y, max) - self.min_quart_y;
        self.quarts[((y * 4 + (quart_x & 3)) * 4 + (quart_z & 3)) as usize]
    }

    fn get_local(&self, x: usize, y: usize, z: usize) -> Biome {
        self.quarts[(y * 4 + x) * 4 + z]
    }
}

struct Samplers {
    final_density: Arc<Sampler>,
    climate: [Arc<Sampler>; 6],
    aquifer: Option<AquiferSamplers>,
}

pub struct NoiseBasedChunkGenerator {
    pub settings_key: String,
    pub settings: Arc<NoiseGeneratorSettings>,
    pub biome_source: BiomeSource,
    pub state: RandomState,
    picker: FluidPicker,
    samplers: Samplers,
    zoom_seed: i64,
    biome_cache: Mutex<HashMap<ChunkPos, Arc<ChunkBiomes>>>,
    carver_biomes: Mutex<HashMap<ChunkPos, Biome>>,
    features_per_step: OnceLock<(Vec<StepFeatureData>, Vec<bool>)>,
}

impl NoiseBasedChunkGenerator {
    pub fn overworld(seed: i64) -> NoiseBasedChunkGenerator {
        NoiseBasedChunkGenerator::with_settings("minecraft:overworld", BiomeSource::MultiNoise(MultiNoiseBiomeSource::from_preset("minecraft:overworld").expect("overworld biome source")), seed)
    }

    pub fn nether(seed: i64) -> NoiseBasedChunkGenerator {
        NoiseBasedChunkGenerator::with_settings("minecraft:nether", BiomeSource::MultiNoise(MultiNoiseBiomeSource::from_preset("minecraft:nether").expect("nether biome source")), seed)
    }

    pub fn end(seed: i64) -> NoiseBasedChunkGenerator {
        NoiseBasedChunkGenerator::with_settings("minecraft:end", BiomeSource::TheEnd(TheEndBiomeSource::create()), seed)
    }

    pub fn with_settings(settings_key: &str, biome_source: BiomeSource, seed: i64) -> NoiseBasedChunkGenerator {
        let mut generator = NoiseBasedChunkGenerator::new(WORLDGEN.noise_settings[settings_key].clone(), biome_source, seed);
        generator.settings_key = settings_key.to_string();
        generator
    }

    pub fn new(settings: Arc<NoiseGeneratorSettings>, biome_source: BiomeSource, seed: i64) -> NoiseBasedChunkGenerator {
        let state = RandomState::new(&settings, seed);
        let router = &settings.noise_router;
        let samplers = Samplers {
            final_density: state.sampler(&router.final_density),
            climate: [&router.temperature, &router.vegetation, &router.continents, &router.erosion, &router.depth, &router.ridges].map(|f| state.sampler(f)),
            aquifer: settings.aquifers.as_ref().map(|config| AquiferSamplers::new(&state, config)),
        };
        NoiseBasedChunkGenerator {
            settings_key: String::new(),
            picker: FluidPicker::new(settings.sea_level, settings.default_fluid),
            zoom_seed: biome::obfuscate_seed(seed),
            biome_cache: Mutex::new(HashMap::new()),
            carver_biomes: Mutex::new(HashMap::new()),
            features_per_step: OnceLock::new(),
            settings,
            biome_source,
            state,
            samplers,
        }
    }

    pub fn features_per_step(&self) -> &(Vec<StepFeatureData>, Vec<bool>) {
        self.features_per_step.get_or_init(|| {
            let possible_biomes = self.biome_source.possible_biomes();
            let mut possible = vec![false; wafer_data::SyncedRegistry::get("minecraft:worldgen/biome").entries.len()];
            for biome in &possible_biomes {
                possible[biome.0 as usize] = true;
            }
            (feature_sorter::build_features_per_step(&possible_biomes, biome_generation_settings::get), possible)
        })
    }

    pub fn seed(&self) -> i64 {
        self.state.seed
    }

    pub fn sea_level(&self) -> i32 {
        self.settings.sea_level
    }

    pub fn min_y(&self) -> i32 {
        self.settings.noise.min_y
    }

    pub fn gen_depth(&self) -> i32 {
        self.settings.noise.height
    }

    pub fn create_biomes(&self, pos: ChunkPos, dimension: &DimensionType) -> ChunkBiomes {
        let mut ctx = SamplerContext::cached();
        let min_quart_x = quart_pos::from_block(pos.min_block_x());
        let min_quart_y = quart_pos::from_block(dimension.min_y);
        let min_quart_z = quart_pos::from_block(pos.min_block_z());
        let size_y = quart_pos::from_block(dimension.height as i32);
        let sections = dimension.height as usize / 16;
        let source = match &self.biome_source {
            BiomeSource::MultiNoise(source) => source,
            BiomeSource::Fixed(biome) => return ChunkBiomes { min_quart_y, quarts: vec![*biome; sections * 64] },
            BiomeSource::Checkerboard(source) => {
                let mut quarts = vec![Biome(0); sections * 64];
                for quart_y in 0..size_y {
                    for x in 0..4 {
                        for z in 0..4 {
                            quarts[((quart_y * 4 + x) * 4 + z) as usize] = source.get_noise_biome(min_quart_x + x, min_quart_z + z);
                        }
                    }
                }
                return ChunkBiomes { min_quart_y, quarts };
            }
            BiomeSource::TheEnd(source) => {
                let mut quarts = vec![Biome(0); sections * 64];
                for quart_y in 0..size_y {
                    for x in 0..4 {
                        for z in 0..4 {
                            quarts[((quart_y * 4 + x) * 4 + z) as usize] = source.get_noise_biome(min_quart_x + x, min_quart_y + quart_y, min_quart_z + z, |bx, by, bz| {
                                self.samplers.climate[3].sample_value(&mut ctx, bx, by, bz) as f64
                            });
                        }
                    }
                }
                return ChunkBiomes { min_quart_y, quarts };
            }
        };
        let volume = DensityVolume::stepped(
            4,
            size_y,
            4,
            quart_pos::to_block(min_quart_x),
            quart_pos::to_block(min_quart_y),
            quart_pos::to_block(min_quart_z),
            4,
            4,
            4,
        );
        let buffers: Vec<Vec<f32>> = self
            .samplers
            .climate
            .iter()
            .map(|sampler| {
                let mut buffer = vec![0.0; volume.size()];
                sampler.sample_volume(&mut ctx, &mut buffer, &volume);
                buffer
            })
            .collect();
        let mut quarts = vec![Biome(0); sections * 64];
        for section in 0..sections as i32 {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        let quart_y = section * 4 + y;
                        let index = volume.index(x, quart_y, z);
                        let target = climate::target(
                            buffers[0][index],
                            buffers[1][index],
                            buffers[2][index],
                            buffers[3][index],
                            buffers[4][index],
                            buffers[5][index],
                        );
                        quarts[((quart_y * 4 + x) * 4 + z) as usize] = source.get_noise_biome(&target);
                    }
                }
            }
        }
        ChunkBiomes { min_quart_y, quarts }
    }

    pub fn chunk_biomes(&self, pos: ChunkPos, dimension: &DimensionType) -> Arc<ChunkBiomes> {
        if let Some(biomes) = self.biome_cache.lock().unwrap().get(&pos) {
            return biomes.clone();
        }
        let biomes = Arc::new(self.create_biomes(pos, dimension));
        let mut cache = self.biome_cache.lock().unwrap();
        if cache.len() >= MAX_CACHED_BIOMES {
            cache.clear();
        }
        cache.entry(pos).or_insert(biomes).clone()
    }

    pub fn generate(&self, pos: ChunkPos, dimension: &DimensionType) -> ProtoChunk {
        self.generate_with_beardifier(pos, dimension, None)
    }

    pub fn generate_with_beardifier(&self, pos: ChunkPos, dimension: &DimensionType, beardifier: Option<Arc<Beardifier>>) -> ProtoChunk {
        let region: Vec<Arc<ChunkBiomes>> =
            (0..9).map(|i| self.chunk_biomes(ChunkPos::new(pos.x + i % 3 - 1, pos.z + i / 3 - 1), dimension)).collect();
        self.build_terrain(pos, dimension, std::array::from_fn(|i| &*region[i]), beardifier, &Arc::new(Blender::empty()))
    }

    pub fn generate_blended(&self, pos: ChunkPos, dimension: &DimensionType, beardifier: Option<Arc<Beardifier>>, biomes: &[Arc<ChunkBiomes>; 9], blender: &Arc<Blender>) -> ProtoChunk {
        self.build_terrain(pos, dimension, std::array::from_fn(|i| &*biomes[i]), beardifier, blender)
    }

    pub fn create_biomes_blended(&self, pos: ChunkPos, dimension: &DimensionType, blender: &Blender) -> Arc<ChunkBiomes> {
        let noise = self.chunk_biomes(pos, dimension);
        if blender.is_empty() {
            return noise;
        }
        let mut quarts = noise.quarts.clone();
        let (min_quart_x, min_quart_z) = (quart_pos::from_block(pos.min_block_x()), quart_pos::from_block(pos.min_block_z()));
        for quart_y in 0..(quarts.len() / 16) as i32 {
            for x in 0..4 {
                for z in 0..4 {
                    let index = ((quart_y * 4 + x) * 4 + z) as usize;
                    if let Some(biome) = blender.blend_biome(min_quart_x + x, noise.min_quart_y + quart_y, min_quart_z + z) {
                        quarts[index] = biome;
                    }
                }
            }
        }
        Arc::new(ChunkBiomes { min_quart_y: noise.min_quart_y, quarts })
    }

    pub fn fill_biomes_upgrading(&self, chunk: &mut LevelChunk, dimension: &DimensionType, blender: &Blender) {
        let pos = chunk.pos;
        let noise = self.chunk_biomes(pos, dimension);
        let min_section = chunk.min_y() >> 4;
        let (min_quart_x, min_quart_z) = (quart_pos::from_block(pos.min_block_x()), quart_pos::from_block(pos.min_block_z()));
        for section_y in below_zero_retrogen::UPGRADE_MIN_Y >> 4..=(below_zero_retrogen::UPGRADE_MIN_Y + below_zero_retrogen::UPGRADE_HEIGHT - 1) >> 4 {
            let index = (section_y - min_section) as usize;
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        let (quart_x, quart_y, quart_z) = (min_quart_x + x, section_y * 4 + y, min_quart_z + z);
                        let noise_biome = blender.blend_biome(quart_x, quart_y, quart_z).unwrap_or_else(|| noise.get(quart_x, quart_y, quart_z));
                        let biome = if BelowZeroRetrogen::is_retained_biome(noise_biome) { noise_biome } else { chunk.noise_biome(quart_x, 0, quart_z) };
                        chunk.sections[index].biomes.set(x as usize, y as usize, z as usize, biome);
                    }
                }
            }
        }
    }

    pub fn fill_biomes_from_noise(sections: &mut [LevelChunkSection], biomes: &ChunkBiomes) {
        for (i, section) in sections.iter_mut().enumerate() {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        section.biomes.set(x, y, z, biomes.get_local(x, i * 4 + y, z));
                    }
                }
            }
        }
    }

    pub fn build_terrain(&self, pos: ChunkPos, dimension: &DimensionType, biomes: [&ChunkBiomes; 9], beardifier: Option<Arc<Beardifier>>, blender: &Arc<Blender>) -> ProtoChunk {
        let mut chunk = ProtoChunk::new(pos, dimension);
        NoiseBasedChunkGenerator::fill_biomes_from_noise(&mut chunk.sections, biomes[4]);
        chunk.status = ChunkStatus::Biomes;
        if self.fill_terrain(&mut chunk, biomes, beardifier, blender) {
            chunk.prime_heightmaps(&HeightmapType::LIVE);
            chunk.status = ChunkStatus::Terrain;
        }
        chunk
    }

    pub fn build_terrain_upgrading(&self, level_chunk: &mut LevelChunk, beardifier: Option<Arc<Beardifier>>, region: &[Arc<ChunkBiomes>; 9], blender: &Arc<Blender>) {
        let mut chunk = ProtoChunk::upgrading_from(level_chunk);
        chunk.status = ChunkStatus::Biomes;
        let biomes: [&ChunkBiomes; 9] = std::array::from_fn(|i| &*region[i]);
        self.fill_terrain(&mut chunk, biomes, beardifier, blender);
        if let Some(retrogen) = level_chunk.proto.as_ref().and_then(|proto| proto.below_zero_retrogen.clone()) {
            BelowZeroRetrogen::replace_old_bedrock(&mut chunk);
            if retrogen.has_bedrock_holes() {
                retrogen.apply_bedrock_mask(&mut chunk);
            }
        }
        chunk.prime_heightmaps(&HeightmapType::LIVE);
        chunk.return_to(level_chunk);
    }

    fn fill_terrain(&self, chunk: &mut ProtoChunk, biomes: [&ChunkBiomes; 9], beardifier: Option<Arc<Beardifier>>, blender: &Arc<Blender>) -> bool {
        let pos = chunk.pos;
        let generation_height = chunk.generation_max_y() - chunk.generation_min_y() + 1;
        let noise = self.settings.noise.clamp_to(chunk.generation_min_y(), generation_height);
        if noise.height <= 0 {
            return false;
        }
        let volume = DensityVolume::new(16, noise.height, 16, pos.min_block_x(), noise.min_y, pos.min_block_z());
        let mut ctx = SamplerContext::cached();
        ctx.beardifier = beardifier;
        if !blender.is_empty() {
            ctx.blend = Some(Arc::new(blender.blend_offset_and_factor_volume(&volume)));
        }
        let mut aquifer = self.create_aquifer(&mut ctx, &volume);
        self.do_fill(chunk, &mut ctx, &mut aquifer, &volume);

        let mut possible = vec![false; wafer_data::SyncedRegistry::get("minecraft:worldgen/biome").entries.len()];
        for chunk_biomes in biomes {
            for biome in &chunk_biomes.quarts {
                possible[biome.0 as usize] = true;
            }
        }
        let zoom_seed = self.zoom_seed;
        let (center_x, center_z) = (pos.x, pos.z);
        let mut biome_getter = |x: i32, y: i32, z: i32| {
            let (qx, qy, qz) = biome::zoomed_quart(zoom_seed, x, y, z);
            let (cx, cz) = ((qx >> 2) - center_x, (qz >> 2) - center_z);
            biomes[((cz + 1) * 3 + cx + 1) as usize].get(qx, qy, qz)
        };
        let generation = WorldGenerationContext {
            min_gen_y: chunk.generation_min_y().max(self.min_y()),
            gen_depth: generation_height.min(self.gen_depth()),
            sea_level: self.sea_level(),
        };
        self.state.material_system().build_surface(
            &self.state,
            chunk,
            &mut ctx,
            volume,
            &mut biome_getter,
            generation,
            &self.settings.material_rule,
            Some(&possible),
        );
        self.generate_carvers(chunk, &mut ctx, &mut aquifer, &mut biome_getter, generation, blender.get_carving_filter());
        true
    }

    fn generate_carvers(
        &self,
        chunk: &mut ProtoChunk,
        ctx: &mut SamplerContext,
        aquifer: &mut Aquifer,
        biome_getter: &mut dyn FnMut(i32, i32, i32) -> Biome,
        generation: WorldGenerationContext,
        filter: Option<&CarvingFilter>,
    ) {
        let pos = chunk.pos;
        let mut random = WorldgenRandom::legacy(0);
        let protected_blocks_on_top = if chunk.upgrading { 0 } else { PROTECTED_BLOCKS_ON_TOP };
        let max_y = generation.min_gen_y + generation.gen_depth - 1 - protected_blocks_on_top;
        let mut mask = CarvingMask::new(generation.min_gen_y + 1, max_y);
        let mut climate_ctx = SamplerContext::uncached();
        for dx in -CARVER_RANGE..=CARVER_RANGE {
            for dz in -CARVER_RANGE..=CARVER_RANGE {
                let source = ChunkPos::new(pos.x + dx, pos.z + dz);
                let biome = self.carver_biome(&mut climate_ctx, source);
                for (index, carver) in BIOME_CARVERS[biome.0 as usize].iter().enumerate() {
                    random.set_large_feature_seed(self.seed().wrapping_add(index as i64), source.x, source.z);
                    if carver.is_start_chunk(&mut random) {
                        carver.carve(&generation, &mut random, pos, source, &mut mask);
                    }
                }
            }
        }
        if !mask.is_empty() {
            self.apply_carving_mask(chunk, &mask, ctx, aquifer, biome_getter, generation, filter);
        }
    }

    fn carver_biome(&self, ctx: &mut SamplerContext, chunk: ChunkPos) -> Biome {
        if let Some(&biome) = self.carver_biomes.lock().unwrap().get(&chunk) {
            return biome;
        }
        let biome = self.get_noise_biome(ctx, quart_pos::from_block(chunk.min_block_x()), 0, quart_pos::from_block(chunk.min_block_z()));
        let mut cache = self.carver_biomes.lock().unwrap();
        if cache.len() >= MAX_CACHED_BIOMES {
            cache.clear();
        }
        cache.insert(chunk, biome);
        biome
    }

    fn apply_carving_mask(
        &self,
        chunk: &mut ProtoChunk,
        mask: &CarvingMask,
        ctx: &mut SamplerContext,
        aquifer: &mut Aquifer,
        biome_getter: &mut dyn FnMut(i32, i32, i32) -> Biome,
        generation: WorldGenerationContext,
        filter: Option<&CarvingFilter>,
    ) {
        let (min_x, min_z) = (chunk.pos.min_block_x(), chunk.pos.min_block_z());
        mask.visit(|x, z, bottom_y, top_y| {
            let mut has_grass = false;
            let (world_x, world_z) = (min_x + x, min_z + z);
            for world_y in (bottom_y..=top_y).rev() {
                if filter.is_some_and(|filter| !filter.test(x, world_y, z)) {
                    continue;
                }
                let old = chunk.block_state(x, world_y, z);
                if old.is(&block_tags::UNCARVABLE) {
                    continue;
                }
                if old.is_block(blocks::GRASS_BLOCK) || old.is_block(blocks::MYCELIUM) {
                    has_grass = true;
                }
                let Some(state) = aquifer.compute_substance(ctx, world_x, world_y, world_z, 0.0) else {
                    continue;
                };
                chunk.set_block_state(x, world_y, z, state);
                if aquifer.should_schedule_fluid_update() && !state.fluid_state().is_empty() {
                    chunk.mark_pos_for_post_processing(world_x, world_y, world_z);
                }
                if has_grass && chunk.block_state(x, world_y - 1, z).is_block(blocks::DIRT) {
                    let under_fluid = !state.fluid_state().is_empty();
                    let top = self.state.material_system().top_material(
                        &self.settings.material_rule,
                        &self.state,
                        generation,
                        biome_getter,
                        chunk,
                        ctx,
                        (world_x, world_y - 1, world_z),
                        under_fluid,
                    );
                    if let Some(top) = top {
                        chunk.set_block_state(x, world_y - 1, z, top);
                        if !top.fluid_state().is_empty() {
                            chunk.mark_pos_for_post_processing(world_x, world_y - 1, world_z);
                        }
                    }
                }
            }
        });
    }

    fn do_fill(&self, chunk: &mut ProtoChunk, ctx: &mut SamplerContext, aquifer: &mut Aquifer, volume: &DensityVolume) {
        chunk.heightmap_mut(HeightmapType::OceanFloorWg);
        chunk.heightmap_mut(HeightmapType::WorldSurfaceWg);
        let mut densities = ctx.acquire(volume.size());
        self.samplers.final_density.sample_volume(ctx, &mut densities, volume);
        let air = blocks::AIR.default_state();
        let default_block = self.settings.default_block;
        for z in 0..volume.size_z {
            let block_z = volume.block_z(z);
            for x in 0..volume.size_x {
                let block_x = volume.block_x(x);
                for y in (0..volume.size_y).rev() {
                    let block_y = volume.block_y(y);
                    let density = densities[volume.index(x, y, z)];
                    let state = aquifer.compute_substance(ctx, block_x, block_y, block_z, density as f64).unwrap_or(default_block);
                    if state != air {
                        self.place(chunk, x, block_y, z, state);
                        if aquifer.should_schedule_fluid_update() && !state.fluid_state().is_empty() {
                            chunk.mark_pos_for_post_processing(block_x, block_y, block_z);
                        }
                    }
                }
            }
        }
        ctx.release(densities);
    }

    fn place(&self, chunk: &mut ProtoChunk, x: i32, y: i32, z: i32, state: BlockState) {
        let index = chunk.section_index(y);
        chunk.sections[index].set_block_state(x as usize, (y & 15) as usize, z as usize, state);
        chunk.update_heightmap(HeightmapType::OceanFloorWg, x, y, z, state);
        chunk.update_heightmap(HeightmapType::WorldSurfaceWg, x, y, z, state);
    }

    pub fn find_biome_horizontal(&self, origin_x: i32, origin_y: i32, origin_z: i32, search_radius: i32, allowed: &crate::core::holder_set::HolderSet, random: &mut impl RandomSource) -> Option<(i32, i32, i32)> {
        let noise_center_x = quart_pos::from_block(origin_x);
        let noise_center_z = quart_pos::from_block(origin_z);
        let noise_radius = quart_pos::from_block(search_radius);
        let noise_y = quart_pos::from_block(origin_y);
        let mut ctx = SamplerContext::cached();
        let mut result = None;
        let mut found = 0;
        for z in -noise_radius..=noise_radius {
            for x in -noise_radius..=noise_radius {
                let noise_x = noise_center_x + x;
                let noise_z = noise_center_z + z;
                let biome = self.get_noise_biome(&mut ctx, noise_x, noise_y, noise_z);
                if allowed.contains(biome.0 as i32) {
                    if result.is_none() || random.next_int_bounded(found + 1) == 0 {
                        result = Some((quart_pos::to_block(noise_x), origin_y, quart_pos::to_block(noise_z)));
                    }
                    found += 1;
                }
            }
        }
        result
    }

    pub fn get_noise_biome(&self, ctx: &mut SamplerContext, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        match &self.biome_source {
            BiomeSource::MultiNoise(source) => {
                let (block_x, block_y, block_z) = (quart_pos::to_block(quart_x), quart_pos::to_block(quart_y), quart_pos::to_block(quart_z));
                let [temperature, humidity, continentalness, erosion, depth, weirdness] =
                    self.samplers.climate.each_ref().map(|sampler| sampler.sample_value(ctx, block_x, block_y, block_z));
                source.get_noise_biome(&climate::target(temperature, humidity, continentalness, erosion, depth, weirdness))
            }
            BiomeSource::TheEnd(source) => source.get_noise_biome(quart_x, quart_y, quart_z, |x, y, z| self.samplers.climate[3].sample_value(ctx, x, y, z) as f64),
            BiomeSource::Fixed(biome) => *biome,
            BiomeSource::Checkerboard(source) => source.get_noise_biome(quart_x, quart_z),
        }
    }

    pub fn get_base_height(&self, x: i32, z: i32, kind: HeightmapType, min_y: i32, height: i32) -> i32 {
        self.iterate_noise_column(min_y, height, x, z, |state| kind.is_opaque(state)).unwrap_or(min_y)
    }

    pub fn get_first_occupied_height(&self, x: i32, z: i32, kind: HeightmapType, min_y: i32, height: i32) -> i32 {
        self.get_base_height(x, z, kind, min_y, height) - 1
    }

    pub fn get_base_column(&self, x: i32, z: i32, min_y: i32, height: i32) -> NoiseColumn {
        let noise = self.settings.noise.clamp_to(min_y, height);
        let mut column = Vec::with_capacity(noise.height.max(0) as usize);
        self.iterate_noise_column(min_y, height, x, z, |state| {
            column.push(state);
            false
        });
        column.reverse();
        NoiseColumn::new(noise.min_y, column)
    }

    fn iterate_noise_column(&self, min_y: i32, height: i32, block_x: i32, block_z: i32, mut tester: impl FnMut(BlockState) -> bool) -> Option<i32> {
        let noise = self.settings.noise.clamp_to(min_y, height);
        if noise.height <= 0 {
            return None;
        }
        let volume = DensityVolume::new(1, noise.height, 1, block_x, noise.min_y, block_z);
        let mut ctx = SamplerContext::cached();
        let mut aquifer = self.create_aquifer(&mut ctx, &volume);
        let default_block = self.settings.default_block;
        let mut densities = ctx.acquire(volume.size());
        self.samplers.final_density.sample_volume(&mut ctx, &mut densities, &volume);
        let mut result = None;
        for y in (0..volume.size_y).rev() {
            let density = densities[volume.index(0, y, 0)];
            let block_y = volume.block_y(y);
            let state = aquifer.compute_substance(&mut ctx, block_x, block_y, block_z, density as f64).unwrap_or(default_block);
            if tester(state) {
                result = Some(block_y + 1);
                break;
            }
        }
        ctx.release(densities);
        result
    }

    pub fn origin(&self) -> ChunkPos {
        if self.settings.spawn_target.is_empty() {
            return ChunkPos::new(0, 0);
        }
        let mut ctx = SamplerContext::cached();
        let targets: Vec<Vec<(Arc<Sampler>, Parameter)>> = self
            .settings
            .spawn_target
            .iter()
            .map(|point| point.parameters.iter().map(|(f, p)| (self.state.sampler(&inline(f)), *p)).collect())
            .collect();
        let (x, z) = find_spawn_position(&mut ctx, &targets);
        ChunkPos::new(x >> 4, z >> 4)
    }

    fn create_aquifer(&self, ctx: &mut SamplerContext, volume: &DensityVolume) -> Aquifer {
        match &self.samplers.aquifer {
            Some(samplers) => {
                let random = self.state.random_factory("minecraft:aquifer");
                Aquifer::NoiseBased(Box::new(NoiseBasedAquifer::new(ctx, samplers.clone(), random, volume, self.picker)))
            }
            None => Aquifer::Disabled(self.picker),
        }
    }
}

fn inline(function: &Df) -> Df {
    match &**function {
        super::density_function::DensityFunction::Reference(r) => r.value.clone(),
        _ => function.clone(),
    }
}

fn find_spawn_position(ctx: &mut SamplerContext, targets: &[Vec<(Arc<Sampler>, Parameter)>]) -> (i32, i32) {
    let fitness = |ctx: &mut SamplerContext, x: i32, z: i32| -> i64 {
        let (qx, qz) = (quart_pos::to_block(quart_pos::from_block(x)), quart_pos::to_block(quart_pos::from_block(z)));
        let mut min_fitness = i64::MAX;
        for point in targets {
            let mut fitness = 0i64;
            for (sampler, parameter) in point {
                let value = climate::quantize_coord(sampler.sample_value(ctx, qx, 0, qz));
                let d = parameter.distance(value);
                fitness = fitness.wrapping_add(d.wrapping_mul(d));
            }
            min_fitness = min_fitness.min(fitness);
        }
        let bias = (x as i64) * (x as i64) + (z as i64) * (z as i64);
        min_fitness.wrapping_mul(2048 * 2048).wrapping_add(bias)
    };
    let mut best = ((0, 0), fitness(ctx, 0, 0));
    for (max_radius, increment) in [(2048.0f32, 512.0f32), (512.0, 32.0)] {
        let mut angle = 0.0f32;
        let mut radius = increment;
        let origin = best.0;
        while radius <= max_radius {
            let x = origin.0 + ((angle as f64).sin() * radius as f64) as i32;
            let z = origin.1 + ((angle as f64).cos() * radius as f64) as i32;
            let candidate = fitness(ctx, x, z);
            if candidate < best.1 {
                best = ((x, z), candidate);
            }
            angle += increment / radius;
            if angle as f64 > std::f64::consts::PI * 2.0 {
                angle = 0.0;
                radius += increment;
            }
        }
    }
    best.0
}
