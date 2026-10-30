use std::cell::RefCell;
use std::sync::{Arc, TryLockError};
use crate::world::level::levelgen::below_zero_retrogen;

use wafer_core::{AABB, BlockPos, ChunkPos, Vec3};
use wafer_data::{Block, BlockState, DimensionType, Fluid, FluidState, blocks};

use crate::world::attribute::environment_attribute_system::EnvironmentAccess;
use crate::world::attribute::{AttributeValue, EnvironmentAttribute, EnvironmentAttributeSystem};
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::LevelEntity;
use crate::world::level::collision::for_each_block_collision;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::scores::scoreboard::Scoreboard;
use super::SoundSource;
use crate::world::level::biome;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::chunk::{Biome, ChunkGenerator, HeightmapType, LevelChunk};
use crate::world::level::levelgen::AlgorithmRandom;
use crate::world::level::levelgen::material::WorldGenerationContext;
use crate::world::level::levelgen::random_state::RandomState;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use crate::world::level::lighting::has_different_light_properties;
use crate::world::level::lighting::level_light_engine::LevelLightEngine;
use crate::world::level::update_flags::UPDATE_KNOWN_SHAPE;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader, LevelWriter, ScheduledTickAccess};
use crate::world::ticks::TickPriority;

const WORLDGEN_REGION_RANDOM: &str = "minecraft:worldgen_region_random";

pub struct WorldGenRegion<'a> {
    cache: [Option<&'a mut LevelChunk>; 9],
    writable: [bool; 9],
    lit: [bool; 9],
    center: ChunkPos,
    dimension: &'static DimensionType,
    seed: i64,
    biome_zoom_seed: i64,
    sea_level: i32,
    min_y: i32,
    height: i32,
    generation: WorldGenerationContext,
    random: AlgorithmRandom,
    write_radius: i32,
    center_upgrading: bool,
    light_engine: &'a LevelLightEngine,
    generator: &'a ChunkGenerator,
    difficulty: Difficulty,
    pub level_data: RegionLevelData,
    pub environment_attributes: Option<&'a EnvironmentAttributeSystem>,
    pub light_tasks: Vec<RegionLightTask>,
    pub poi_updates: Vec<(BlockPos, BlockState, BlockState)>,
    pub entity_teams: Vec<(u128, String)>,
    pub entity_sounds: Vec<(Vec3, i32, SoundSource, f32, f32)>,
    pub scoreboard: Option<&'a RefCell<Scoreboard>>,
    pub structure_starts: Option<Arc<Vec<Vec<Arc<StructureStart>>>>>,
}

#[derive(Clone, Copy, Debug)]
pub struct RegionLevelData {
    pub game_time: i64,
    pub overworld_clock_time: i64,
    pub moon_brightness: f32,
    pub thundering: bool,
    pub world_border: [f64; 4],
}

impl Default for RegionLevelData {
    fn default() -> RegionLevelData {
        RegionLevelData { game_time: 0, overworld_clock_time: 0, moon_brightness: 0.0, thundering: false, world_border: [f64::MIN, f64::MIN, f64::MAX, f64::MAX] }
    }
}

pub enum RegionLightTask {
    UpdateSectionStatus(BlockPos, bool),
    CheckBlock(BlockPos),
}

impl<'a> WorldGenRegion<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        center: ChunkPos,
        cache: [Option<&'a mut LevelChunk>; 9],
        writable: [bool; 9],
        lit: [bool; 9],
        generator: &'a ChunkGenerator,
        random_state: &RandomState,
        dimension: &'static DimensionType,
        generation: WorldGenerationContext,
        light_engine: &'a LevelLightEngine,
        difficulty: Difficulty,
    ) -> WorldGenRegion<'a> {
        let seed = random_state.seed;
        let random = random_state.random_factory(WORLDGEN_REGION_RANDOM).at(center.min_block_x(), 0, center.min_block_z());
        let center_upgrading = cache[4].as_ref().and_then(|chunk| chunk.proto.as_ref()).is_some_and(|proto| proto.below_zero_retrogen.is_some());
        WorldGenRegion {
            cache,
            writable,
            lit,
            center,
            dimension,
            seed,
            biome_zoom_seed: biome::obfuscate_seed(seed),
            sea_level: generation.sea_level,
            min_y: dimension.min_y,
            height: dimension.height as i32,
            generation,
            random,
            write_radius: 1,
            center_upgrading,
            light_engine,
            generator,
            difficulty,
            level_data: RegionLevelData::default(),
            environment_attributes: None,
            light_tasks: Vec::new(),
            poi_updates: Vec::new(),
            entity_teams: Vec::new(),
            entity_sounds: Vec::new(),
            scoreboard: None,
            structure_starts: None,
        }
    }

    pub fn generator(&self) -> &'a ChunkGenerator {
        self.generator
    }

    pub fn get_difficulty(&self) -> Difficulty {
        self.difficulty
    }

    fn index(&self, chunk_x: i32, chunk_z: i32) -> usize {
        let (dx, dz) = (chunk_x - self.center.x, chunk_z - self.center.z);
        assert!(dx.abs() <= 1 && dz.abs() <= 1, "Requested chunk unavailable during world generation: {chunk_x}, {chunk_z} around {:?}", self.center);
        ((dz + 1) * 3 + dx + 1) as usize
    }

    pub fn get_center(&self) -> ChunkPos {
        self.center
    }

    pub fn generation(&self) -> WorldGenerationContext {
        self.generation
    }

    pub fn get_seed(&self) -> i64 {
        self.seed
    }

    pub fn get_sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn get_min_y(&self) -> i32 {
        self.min_y
    }

    pub fn get_height_of_level(&self) -> i32 {
        self.height
    }

    pub fn get_max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.min_y || y >= self.min_y + self.height
    }

    pub fn get_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> &mut LevelChunk {
        let index = self.index(chunk_x, chunk_z);
        self.cache[index].as_deref_mut().expect("a chunk in the region")
    }

    pub fn has_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        (chunk_x - self.center.x).abs() <= 1 && (chunk_z - self.center.z).abs() <= 1 && self.cache[self.index(chunk_x, chunk_z)].is_some()
    }

    fn chunk_for_collisions(&self, pos: ChunkPos) -> Option<&LevelChunk> {
        if self.has_chunk(pos.x, pos.z) { self.cache[self.index(pos.x, pos.z)].as_deref() } else { None }
    }

    fn chunk(&self, chunk_x: i32, chunk_z: i32) -> &LevelChunk {
        self.cache[self.index(chunk_x, chunk_z)].as_deref().expect("a chunk in the region")
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        if self.is_outside_build_height(pos.y) {
            return blocks::VOID_AIR.default_state();
        }
        self.chunk(pos.x >> 4, pos.z >> 4).block_state(pos)
    }

    pub fn get_fluid_state(&self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    pub fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        self.get_chunk(x >> 4, z >> 4).get_height(kind, x & 15, z & 15) + 1
    }

    pub fn get_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        self.chunk(quart_x >> 2, quart_z >> 2).noise_biome(quart_x, quart_y, quart_z)
    }

    pub fn get_biome(&self, pos: BlockPos) -> Biome {
        let (x, y, z) = biome::zoomed_quart(self.biome_zoom_seed, pos.x, pos.y, pos.z);
        self.get_noise_biome(x, y, z)
    }

    fn is_within_write_zone(&self, chunk_x: i32, chunk_z: i32) -> bool {
        (self.center.x - chunk_x).abs() <= self.write_radius && (self.center.z - chunk_z).abs() <= self.write_radius
    }

    pub fn ensure_can_write(&self, pos: BlockPos) -> bool {
        if !self.is_within_write_zone(pos.x >> 4, pos.z >> 4) {
            return false;
        }
        !self.center_upgrading || (below_zero_retrogen::UPGRADE_MIN_Y..below_zero_retrogen::UPGRADE_MIN_Y + below_zero_retrogen::UPGRADE_HEIGHT).contains(&pos.y)
    }

    pub fn center_generation_bounds(&self) -> (i32, i32) {
        if self.center_upgrading {
            (below_zero_retrogen::UPGRADE_MIN_Y, below_zero_retrogen::UPGRADE_MIN_Y + below_zero_retrogen::UPGRADE_HEIGHT - 1)
        } else {
            (self.min_y, self.min_y + self.height - 1)
        }
    }

    pub fn is_center_upgrading(&self) -> bool {
        self.center_upgrading
    }

    pub fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool {
        if !self.ensure_can_write(pos) {
            return false;
        }
        let index = self.index(pos.x >> 4, pos.z >> 4);
        if !self.writable[index] {
            return true;
        }
        let outside = self.is_outside_build_height(pos.y);
        let lit = self.lit[index];
        let chunk = self.cache[index].as_deref_mut().expect("a chunk in the region");
        let old = if outside {
            Some(blocks::VOID_AIR.default_state())
        } else {
            let before = chunk.block_state(pos);
            let section = ((pos.y - chunk.min_y()) >> 4) as usize;
            let was_empty = chunk.sections[section].has_only_air();
            if chunk.set_block_state(pos, state).is_some() && lit {
                let is_empty = chunk.sections[section].has_only_air();
                if is_empty != was_empty {
                    self.light_tasks.push(RegionLightTask::UpdateSectionStatus(pos, is_empty));
                }
                if has_different_light_properties(before, state) {
                    let chunk = self.cache[index].as_deref_mut().expect("a chunk in the region");
                    chunk.update_sky_light_sources(pos);
                    self.light_tasks.push(RegionLightTask::CheckBlock(pos));
                }
            }
            Some(before)
        };
        if let Some(old) = old {
            self.poi_updates.push((pos, old, state));
        }
        let chunk = self.cache[index].as_deref_mut().expect("a chunk in the region");
        if state.has_block_entity() {
            chunk.block_entities.pending.insert(pos, dummy_block_entity(pos));
        } else if old.is_some_and(|old| old.has_block_entity()) {
            chunk.block_entities.pending.remove(&pos);
            chunk.block_entities.map.remove(&pos);
        }
        if flags & UPDATE_KNOWN_SHAPE == 0
            && let Some(post_process_pos) = post_process_pos(state, pos)
        {
            self.mark_pos_for_post_processing(post_process_pos);
        }
        true
    }

    pub fn set_section_block_state(&mut self, pos: BlockPos, state: BlockState) {
        let chunk = self.get_chunk(pos.x >> 4, pos.z >> 4);
        let index = ((pos.y - chunk.min_y()) >> 4) as usize;
        chunk.sections[index].set_block_state((pos.x & 15) as usize, (pos.y & 15) as usize, (pos.z & 15) as usize, state);
    }

    pub fn set_block_and_update(&mut self, pos: BlockPos, state: BlockState) -> bool {
        self.set_block(pos, state, crate::world::level::update_flags::UPDATE_ALL)
    }

    pub fn mark_pos_for_post_processing(&mut self, pos: BlockPos) {
        self.get_chunk(pos.x >> 4, pos.z >> 4).mark_pos_for_post_processing(pos);
    }

    pub fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut BlockEntity> {
        let state = self.get_block_state(pos);
        let chunk = self.get_chunk(pos.x >> 4, pos.z >> 4);
        if !chunk.block_entities.map.contains_key(&pos)
            && let Some(tag) = chunk.block_entities.pending.get(&pos)
        {
            let entity = if tag.get("id").and_then(wafer_protocol::nbt::Nbt::as_str) == Some("DUMMY") {
                if state.has_block_entity() { BlockEntity::new_for(pos, state) } else { None }
            } else {
                BlockEntity::load_static(pos, state, tag, &["net.minecraft.server.level.WorldGenRegion.getBlockEntity(WorldGenRegion.java:247)"])
            };
            if let Some(entity) = entity {
                chunk.block_entities.pending.remove(&pos);
                chunk.set_block_entity(entity);
            }
        }
        chunk.block_entities.map.get_mut(&pos)
    }
}

fn dummy_block_entity(pos: BlockPos) -> wafer_protocol::nbt::Nbt {
    use wafer_protocol::nbt::Nbt;
    Nbt::Compound(vec![("x".into(), Nbt::Int(pos.x)), ("y".into(), Nbt::Int(pos.y)), ("z".into(), Nbt::Int(pos.z)), ("id".into(), Nbt::String("DUMMY".into()))].into())
}

fn post_process_pos(state: BlockState, pos: BlockPos) -> Option<BlockPos> {
    let block = state.block();
    if block == blocks::BROWN_MUSHROOM || block == blocks::RED_MUSHROOM {
        Some(pos)
    } else if block == blocks::SOUL_SAND || block == blocks::MAGMA_BLOCK {
        Some(pos.above())
    } else {
        None
    }
}

impl crate::world::level::world_gen_level::WorldGenLevel for WorldGenRegion<'_> {
    fn get_seed(&self) -> i64 {
        WorldGenRegion::get_seed(self)
    }

    fn ensure_can_write(&self, pos: BlockPos) -> bool {
        WorldGenRegion::ensure_can_write(self, pos)
    }

    fn generator(&self) -> &ChunkGenerator {
        self.generator
    }

    fn generation(&self) -> WorldGenerationContext {
        self.generation
    }

    fn get_block_state(&mut self, pos: BlockPos) -> BlockState {
        WorldGenRegion::get_block_state(self, pos)
    }

    fn load_team(&mut self, entity: &crate::world::entity::Entity, nbt: &wafer_protocol::nbt::Nbt) {
        let Some(team_name) = crate::world::level::storage::tag_value_input::get_string(nbt, "Team") else { return };
        match self.scoreboard {
            Some(scoreboard) if scoreboard.try_borrow_mut().is_ok() => crate::world::entity::entity_save::add_loaded_entity_to_team(scoreboard, entity.uuid, team_name),
            _ => self.entity_teams.push((entity.uuid, team_name.to_string())),
        }
    }

    fn play_entity_sound(&mut self, position: Vec3, sound: i32, source: SoundSource, volume: f32, pitch: f32) {
        self.entity_sounds.push((position, sound, source, volume, pitch));
    }

    fn get_height_of_level(&self) -> i32 {
        self.height
    }

    fn set_block_and_update(&mut self, pos: BlockPos, state: BlockState) -> bool {
        WorldGenRegion::set_block_and_update(self, pos, state)
    }

    fn set_section_block_state(&mut self, pos: BlockPos, state: BlockState) {
        WorldGenRegion::set_section_block_state(self, pos, state);
    }

    fn destroy_block(&mut self, pos: BlockPos, _drop_resources: bool) -> bool {
        if self.get_block_state(pos).is_air() {
            return false;
        }
        self.set_block(pos, blocks::AIR.default_state(), crate::world::level::update_flags::UPDATE_ALL)
    }
}

impl EnvironmentAccess for WorldGenRegion<'_> {
    fn biome(&self, x: i32, y: i32, z: i32) -> Biome {
        WorldGenRegion::get_biome(self, BlockPos::new(x, y, z))
    }

    fn noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        self.get_noise_biome(quart_x, quart_y, quart_z)
    }

    fn clock_total_ticks(&self, _clock: usize) -> i64 {
        0
    }

    fn rain_level(&self) -> f32 {
        0.0
    }

    fn thunder_level(&self) -> f32 {
        0.0
    }
}

impl ServerLevelAccessor for WorldGenRegion<'_> {
    fn get_difficulty(&self) -> Difficulty {
        self.difficulty
    }

    fn get_current_difficulty_at(&mut self, pos: BlockPos) -> DifficultyInstance {
        assert!(self.has_chunk(pos.x >> 4, pos.z >> 4), "We are asking a region for a chunk out of bound");
        DifficultyInstance::new(self.difficulty, self.level_data.overworld_clock_time, 0, self.level_data.moon_brightness)
    }

    fn get_height(&mut self, kind: HeightmapType, x: i32, z: i32) -> i32 {
        WorldGenRegion::get_height(self, kind, x, z)
    }

    fn get_biome(&mut self, pos: BlockPos) -> Biome {
        WorldGenRegion::get_biome(self, pos)
    }

    fn get_environment_attribute<T: AttributeValue>(&mut self, attribute: EnvironmentAttribute<T>, pos: BlockPos) -> T {
        let system = self.environment_attributes.expect("a region with environment attributes");
        system.get_value(attribute, Vec3::at_center_of(pos), self)
    }

    fn dimension_type(&self) -> &'static DimensionType {
        self.dimension
    }

    fn get_sky_darken(&self) -> i32 {
        0
    }

    fn get_game_time(&self) -> i64 {
        self.level_data.game_time
    }

    fn is_thundering(&self) -> bool {
        self.level_data.thundering
    }

    fn is_within_world_border(&self, pos: BlockPos) -> bool {
        let [min_x, min_z, max_x, max_z] = self.level_data.world_border;
        let (x, z) = (pos.x as f64, pos.z as f64);
        x >= min_x && x < max_x && z >= min_z && z < max_z
    }

    fn get_nearest_player(&self, _x: f64, _y: f64, _z: f64, _max_dist: f64, _filter_out_creative: bool) -> Option<crate::world::entity::EntityId> {
        None
    }

    fn no_collision(&mut self, aabb: &AABB) -> bool {
        for_each_block_collision(|pos| self.chunk_for_collisions(pos), aabb, &CollisionContext::EMPTY, false, false, |_, c| c.shape.is_empty())
    }

    fn is_unobstructed(&mut self, _aabb: &AABB) -> bool {
        true
    }

    fn add_fresh_entity(&mut self, entity: LevelEntity) -> bool {
        let block_pos = entity.entity().block_position();
        let chunk = self.get_chunk(block_pos.x >> 4, block_pos.z >> 4);
        if let Some(proto) = chunk.proto.as_mut()
            && let Some(tag) = entity.save()
        {
            proto.entities.push(tag);
        }
        true
    }

    fn get_structure_with_piece_at(&mut self, pos: BlockPos, predicate: &dyn Fn(StructureHolder) -> bool) -> Option<Arc<StructureStart>> {
        let structure_starts = self.structure_starts.as_ref().filter(|_| pos.x >> 4 == self.center.x && pos.z >> 4 == self.center.z)?;
        for (structure, starts) in structure_starts.iter().enumerate() {
            if !predicate(StructureHolder(structure as u16)) {
                continue;
            }
            if let Some(start) = starts.iter().find(|start| structure_has_piece_at(pos, start)) {
                return Some(start.clone());
            }
        }
        None
    }
}

fn structure_has_piece_at(pos: BlockPos, structure_start: &StructureStart) -> bool {
    match structure_start.piece_container.try_lock() {
        Ok(container) => container.is_inside_piece(pos),
        Err(TryLockError::Poisoned(container)) => container.into_inner().is_inside_piece(pos),
        Err(TryLockError::WouldBlock) => true,
    }
}

impl BlockGetter for WorldGenRegion<'_> {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        self.get_block_state(pos)
    }

    fn get_min_y(&self) -> i32 {
        self.min_y
    }
}

impl LevelReader for WorldGenRegion<'_> {
    fn raw_brightness(&mut self, pos: BlockPos, darkening: i32) -> i32 {
        self.light_engine.raw_brightness(pos, darkening)
    }

    fn get_brightness(&mut self, layer: crate::world::level::light_layer::LightLayer, pos: BlockPos) -> i32 {
        self.light_engine.light_value(layer, pos)
    }

    fn get_sea_level(&self) -> i32 {
        self.sea_level
    }

    fn is_inside_build_height(&self, y: i32) -> bool {
        !self.is_outside_build_height(y)
    }

    fn is_fast_lava(&self) -> bool {
        false
    }

    fn get_control_input_signal(&mut self, pos: BlockPos, direction: wafer_core::Direction, only_diodes: bool) -> i32 {
        crate::world::level::signal::get_control_input_signal(self, pos, direction, only_diodes)
    }

    fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut BlockEntity> {
        WorldGenRegion::get_block_entity(self, pos)
    }
}

impl ScheduledTickAccess for WorldGenRegion<'_> {
    fn schedule_tick_with_priority(&mut self, pos: BlockPos, block: Block, _delay: i32, priority: TickPriority) {
        if let Some(proto) = self.get_chunk(pos.x >> 4, pos.z >> 4).proto.as_deref_mut() {
            proto.block_ticks.schedule(block, pos, priority);
        }
    }

    fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: Fluid, _delay: i32) {
        if let Some(proto) = self.get_chunk(pos.x >> 4, pos.z >> 4).proto.as_deref_mut() {
            proto.fluid_ticks.schedule(fluid, pos, TickPriority::NORMAL);
        }
    }

    fn has_scheduled_tick(&self, pos: BlockPos, block: Block) -> bool {
        self.chunk(pos.x >> 4, pos.z >> 4).proto.as_deref().is_some_and(|proto| proto.block_ticks.has_scheduled_tick(pos, block))
    }
}

impl LevelWriter for WorldGenRegion<'_> {
    fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool {
        WorldGenRegion::set_block(self, pos, state, flags)
    }
}

impl LevelAccessor for WorldGenRegion<'_> {
    fn get_random(&mut self) -> &mut dyn wafer_core::random::RandomSource {
        &mut self.random
    }

    fn mark_pos_for_post_processing(&mut self, pos: BlockPos) {
        WorldGenRegion::mark_pos_for_post_processing(self, pos);
    }
}
