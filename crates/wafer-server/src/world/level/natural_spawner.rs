use std::cell::RefCell;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, ChunkPos, Direction, mth};
use wafer_data::{BlockState, FluidState, block_tags, blocks};

use super::BlockGetter;
use super::local_mob_cap_calculator::LocalMobCapCalculator;
use super::potential_calculator::PotentialCalculator;
use super::server_level_accessor::ServerLevelAccessor;
use super::world_gen_level::WorldGenLevel;
use crate::server::level::ServerLevel;
use crate::util::random::WeightedList;
use crate::util::valueproviders::{IntProvider, random_between_inclusive};
use crate::world::attribute::environment_attributes;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::mob::{self, Mob};
use crate::world::entity::mob_category::MobCategory;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::spawn_placements;
use crate::world::entity::{EntityId, LevelEntity, entity_types};
use crate::world::level::biome::mob_spawn_settings::SpawnerData;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_spawn_override::BoundingBoxType;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

const MIN_SPAWN_DISTANCE: i32 = 24;
pub const SPAWN_DISTANCE_CHUNK: i32 = 8;
pub const SPAWN_DISTANCE_BLOCK: i32 = 128;
pub const INSCRIBED_SQUARE_SPAWN_DISTANCE_CHUNK: i32 = 5;
const MAGIC_NUMBER: i32 = 17 * 17;
const SPAWNING_CATEGORIES: [MobCategory; 7] = [
    MobCategory::Monster,
    MobCategory::Creature,
    MobCategory::Ambient,
    MobCategory::Axolotls,
    MobCategory::UndergroundWaterCreature,
    MobCategory::WaterCreature,
    MobCategory::WaterAmbient,
];

pub type SpawnPredicate<'a> = dyn FnMut(&'static EntityType, &mut ServerLevel, BlockPos, ChunkPos) -> bool + 'a;
pub type AfterSpawnCallback<'a> = dyn FnMut(&mut ServerLevel, EntityId, ChunkPos) + 'a;

static FORTRESS_ENEMIES: std::sync::LazyLock<WeightedList<SpawnerData>> = std::sync::LazyLock::new(|| {
    WeightedList::new(vec![
        (SpawnerData::new(entity_types::BLAZE.id, IntProvider::Uniform { min_inclusive: 2, max_inclusive: 3 }), 10),
        (SpawnerData::new(entity_types::ZOMBIFIED_PIGLIN.id, IntProvider::Constant(4)), 5),
        (SpawnerData::new(entity_types::WITHER_SKELETON.id, IntProvider::Constant(5)), 8),
        (SpawnerData::new(entity_types::SKELETON.id, IntProvider::Constant(5)), 2),
        (SpawnerData::new(entity_types::MAGMA_CUBE.id, IntProvider::Constant(4)), 3),
    ])
});

pub fn create_state(spawnable_chunk_count: i32, level: &mut ServerLevel, mut local_mob_cap_calculator: LocalMobCapCalculator) -> SpawnState {
    let mut spawn_potential = PotentialCalculator::default();
    let mut mob_counts = [0; 8];
    for index in 0..level.entities.len() {
        let (&id, entity) = level.entities.get_index(index).expect("an entity");
        if entity.entity().is_removed() {
            continue;
        }
        let is_mob = match entity {
            LevelEntity::Mob(mob) => {
                if mob.is_persistence_required() || level.requires_custom_persistence(id) {
                    continue;
                }
                true
            }
            _ => false,
        };
        let entity_type = EntityType::by_id(entity.entity().entity_type);
        let category = entity_type.get_category();
        if category == MobCategory::Misc {
            continue;
        }
        let pos = entity.entity().block_position();
        let chunk_pos = ChunkPos::new(pos.x >> 4, pos.z >> 4);
        if level.chunks.loaded_chunk(chunk_pos).is_none() {
            continue;
        }
        let natural_mob_spawns = level.environment_attributes().get_value_at(environment_attributes::NATURAL_MOB_SPAWNS, pos);
        if let Some(mob_spawn_cost) = natural_mob_spawns.get_mob_spawn_cost(entity_type.id) {
            spawn_potential.add_charge(pos, mob_spawn_cost.charge);
        }
        if is_mob {
            local_mob_cap_calculator.add_mob(level, chunk_pos, category);
        }
        mob_counts[category as usize] += 1;
    }
    SpawnState::new(spawnable_chunk_count, mob_counts, spawn_potential, local_mob_cap_calculator)
}

pub fn get_filtered_spawning_categories(state: &SpawnState, spawn_enemies: bool, spawn_persistent: bool) -> Vec<MobCategory> {
    SPAWNING_CATEGORIES
        .into_iter()
        .filter(|&mob_category| {
            (spawn_enemies || mob_category.is_friendly()) && (spawn_persistent || !mob_category.is_persistent()) && state.can_spawn_for_category_global(mob_category)
        })
        .collect()
}

pub fn spawn_for_chunk(level: &mut ServerLevel, chunk: ChunkPos, state: &mut SpawnState, spawning_categories: &[MobCategory]) {
    let state = RefCell::new(state);
    for &mob_category in spawning_categories {
        if state.borrow_mut().can_spawn_for_category_local(level, mob_category, chunk) {
            spawn_category_for_chunk(
                mob_category,
                level,
                chunk,
                &mut |entity_type, level, pos, chunk| state.borrow_mut().can_spawn(entity_type, level, pos, chunk),
                &mut |level, mob, chunk| state.borrow_mut().after_spawn(level, mob, chunk),
            );
        }
    }
}

pub fn spawn_category_for_chunk(mob_category: MobCategory, level: &mut ServerLevel, chunk: ChunkPos, extra_test: &mut SpawnPredicate, spawn_callback: &mut AfterSpawnCallback) {
    let start = get_random_pos_within(level, chunk);
    if start.y >= level.chunks.min_y() + 1 {
        spawn_category_for_position(mob_category, level, chunk, start, extra_test, spawn_callback);
    }
}

pub fn spawn_category_for_position_debug(mob_category: MobCategory, level: &mut ServerLevel, start: BlockPos) {
    let chunk = ChunkPos::new(start.x >> 4, start.z >> 4);
    spawn_category_for_position(mob_category, level, chunk, start, &mut |_, _, _, _| true, &mut |_, _, _| {});
}

pub fn spawn_category_for_position(
    mob_category: MobCategory,
    level: &mut ServerLevel,
    chunk: ChunkPos,
    start: BlockPos,
    extra_test: &mut SpawnPredicate,
    spawn_callback: &mut AfterSpawnCallback,
) {
    let y_start = start.y;
    let state = level.block_state(start);
    if level.is_redstone_conductor_at(start, state) {
        return;
    }
    let mut cluster_size = 0;
    for _group_count in 0..3 {
        let mut x = start.x;
        let mut z = start.z;
        let mut current_spawn_data: Option<SpawnerData> = None;
        let mut group_data: Option<SpawnGroupData> = None;
        let mut max = mth::ceil((level.random.next_float() * 4.0) as f64);
        let mut group_size = 0;
        let mut ll = 0;
        while ll < max {
            x += level.random.next_int_bounded(6) - level.random.next_int_bounded(6);
            z += level.random.next_int_bounded(6) - level.random.next_int_bounded(6);
            let pos = BlockPos::new(x, y_start, z);
            let xx = x as f64 + 0.5;
            let zz = z as f64 + 0.5;
            if let Some(nearest_player) = level.get_nearest_player_to(xx, y_start as f64, zz, -1.0, false) {
                let nearest_player_distance_sqr = level.entity(nearest_player).distance_to_sqr_xyz(xx, y_start as f64, zz);
                if is_right_distance_to_player_and_spawn_point(level, chunk, pos, nearest_player_distance_sqr) {
                    if current_spawn_data.is_none() {
                        let Some(next_spawn_data) = get_random_spawn_mob_at(level, mob_category, pos) else { break };
                        max = next_spawn_data.count.sample(&mut level.random);
                        current_spawn_data = Some(next_spawn_data);
                    }
                    let spawn_data = current_spawn_data.as_ref().expect("spawn data");
                    let entity_type = spawn_data.type_();
                    if is_valid_spawn_postition_for_type(level, mob_category, spawn_data, pos, nearest_player_distance_sqr) && extra_test(entity_type, level, pos, chunk) {
                        let Some(mut mob) = get_mob_for_spawn(entity_type) else { return };
                        let y_rot = level.random.next_float() * 360.0;
                        mob.entity.snap_to(xx, y_start as f64, zz, y_rot, 0.0);
                        if is_valid_position_for_mob(level, &mob, nearest_player_distance_sqr) {
                            let difficulty = level.current_difficulty_at(mob.entity.block_position());
                            let (entity, next_group_data) = level.finalize_spawn_detached(LevelEntity::Mob(mob), &difficulty, EntitySpawnReason::Natural, group_data);
                            group_data = next_group_data;
                            let LevelEntity::Mob(mob) = entity else { unreachable!("a mob") };
                            cluster_size += 1;
                            group_size += 1;
                            let max_spawn_cluster_size = mob.get_max_spawn_cluster_size();
                            let max_group_size_reached = mob.is_max_group_size_reached(group_size);
                            let id = mob.entity.id;
                            level.add_fresh_entity_with_passengers(LevelEntity::Mob(mob));
                            spawn_callback(level, id, chunk);
                            if cluster_size >= max_spawn_cluster_size {
                                return;
                            }
                            if max_group_size_reached {
                                break;
                            }
                        }
                    }
                }
            }
            ll += 1;
        }
    }
}

fn is_right_distance_to_player_and_spawn_point(level: &mut ServerLevel, chunk: ChunkPos, pos: BlockPos, nearest_player_distance_sqr: f64) -> bool {
    if nearest_player_distance_sqr <= (MIN_SPAWN_DISTANCE * MIN_SPAWN_DISTANCE) as f64 {
        return false;
    }
    let respawn_data = &level.respawn_data;
    if respawn_data.dimension == level.dimension
        && respawn_data.pos.dist_to_center_sqr(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5) < (MIN_SPAWN_DISTANCE * MIN_SPAWN_DISTANCE) as f64
    {
        return false;
    }
    let chunk_pos = ChunkPos::new(pos.x >> 4, pos.z >> 4);
    chunk_pos == chunk || level.can_spawn_entities_in_chunk(chunk_pos)
}

fn is_valid_spawn_postition_for_type(
    level: &mut ServerLevel,
    mob_category: MobCategory,
    current_spawn_data: &SpawnerData,
    pos: BlockPos,
    nearest_player_distance_sqr: f64,
) -> bool {
    let entity_type = current_spawn_data.type_();
    let despawn_distance = entity_type.get_category().get_despawn_distance();
    if entity_type.get_category() == MobCategory::Misc {
        false
    } else if !entity_type.can_spawn_far_from_player() && nearest_player_distance_sqr > (despawn_distance * despawn_distance) as f64 {
        false
    } else if !entity_type.can_summon() || !can_spawn_mob_at(level, mob_category, current_spawn_data, pos) {
        false
    } else if !spawn_placements::is_spawn_position_ok(entity_type, level, pos) {
        false
    } else if !spawn_placements::check_spawn_rules(entity_type, level, EntitySpawnReason::Natural, pos) {
        false
    } else {
        ServerLevelAccessor::no_collision(level, &entity_type.get_spawn_aabb(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5))
    }
}

fn get_mob_for_spawn(entity_type: &EntityType) -> Option<Box<Mob>> {
    match ServerLevel::create_entity(entity_type.get_key())? {
        LevelEntity::Mob(mob) => Some(mob),
        _ => {
            warn!("Can't spawn entity of type: {}", entity_type.get_key());
            None
        }
    }
}

fn is_valid_position_for_mob(level: &mut ServerLevel, mob: &Mob, nearest_player_distance_sqr: f64) -> bool {
    let despawn_distance = mob.get_type().get_category().get_despawn_distance();
    if nearest_player_distance_sqr > (despawn_distance * despawn_distance) as f64 && mob.remove_when_far_away(nearest_player_distance_sqr) {
        false
    } else {
        mob::check_spawn_rules(mob, level, EntitySpawnReason::Natural) && mob::check_spawn_obstruction(mob, level)
    }
}

fn get_random_spawn_mob_at(level: &mut ServerLevel, mob_category: MobCategory, pos: BlockPos) -> Option<SpawnerData> {
    let biome = level.get_biome(pos);
    if mob_category == MobCategory::WaterAmbient && biome.is("minecraft:reduced_water_ambient_spawns") && level.random.next_float() < 0.98 {
        return None;
    }
    let mobs = mobs_at(level, mob_category, pos);
    mobs.get_random(&mut level.random).cloned()
}

fn can_spawn_mob_at(level: &mut ServerLevel, mob_category: MobCategory, spawner_data: &SpawnerData, pos: BlockPos) -> bool {
    mobs_at(level, mob_category, pos).contains(spawner_data)
}

fn mobs_at(level: &mut ServerLevel, mob_category: MobCategory, pos: BlockPos) -> WeightedList<SpawnerData> {
    if is_in_nether_fortress_bounds(pos, level, mob_category) { FORTRESS_ENEMIES.clone() } else { get_mobs_at(level, mob_category, pos) }
}

fn get_mobs_at(level: &mut ServerLevel, mob_category: MobCategory, pos: BlockPos) -> WeightedList<SpawnerData> {
    for structure in level.get_all_structures_at(pos) {
        let Some(structure_override) = &structure.value().settings.spawn_overrides[mob_category as usize] else { continue };
        let mut in_override_box = false;
        level.fill_starts_for_structure(pos, structure, &mut |start| {
            if !in_override_box {
                in_override_box = match structure_override.bounding_box {
                    BoundingBoxType::Piece => start.piece_container.lock().unwrap().is_inside_piece(pos),
                    BoundingBoxType::Full => start.get_bounding_box().is_inside(pos),
                };
            }
        });
        if in_override_box {
            return structure_override.spawns.clone();
        }
    }
    level.environment_attributes().get_value_at(environment_attributes::NATURAL_MOB_SPAWNS, pos).get_mobs_to_spawn(mob_category).clone()
}

pub fn is_in_nether_fortress_bounds(pos: BlockPos, level: &mut ServerLevel, category: MobCategory) -> bool {
    if category == MobCategory::Monster && level.block_state(pos.below()).is_block(blocks::NETHER_BRICKS) {
        match StructureHolder::by_name("minecraft:fortress") {
            Some(fortress) => level.get_structure_at(pos, fortress).is_some(),
            None => false,
        }
    } else {
        false
    }
}

fn get_random_pos_within(level: &mut ServerLevel, chunk: ChunkPos) -> BlockPos {
    let x = chunk.min_block_x() + level.random.next_int_bounded(16);
    let z = chunk.min_block_z() + level.random.next_int_bounded(16);
    let top_empty_y = level.chunks.loaded_chunk_mut(chunk).expect("a spawning chunk").get_height(HeightmapType::WorldSurface, x, z) + 1;
    let y = random_between_inclusive(&mut level.random, level.chunks.min_y(), top_empty_y);
    BlockPos::new(x, y, z)
}

pub fn is_valid_empty_spawn_block(level: &mut (impl BlockGetter + ?Sized), pos: BlockPos, block_state: BlockState, fluid_state: FluidState, entity_type: &EntityType) -> bool {
    if level.is_collision_shape_full_block_at(pos, block_state) {
        false
    } else if block_state.is_signal_source() {
        false
    } else if !fluid_state.is_empty() {
        false
    } else if block_state.is(&block_tags::PREVENT_MOB_SPAWNING_INSIDE) {
        false
    } else {
        !entity_type.is_block_dangerous(block_state)
    }
}

pub fn spawn_mobs_for_chunk_generation(level: &mut (impl WorldGenLevel + ?Sized), source_pos: BlockPos, chunk_pos: ChunkPos, random: &mut WorldgenRandom, spawn_mobs: bool) {
    if !spawn_mobs {
        return;
    }
    let mob_settings = level.get_environment_attribute(environment_attributes::NATURAL_MOB_SPAWNS, source_pos);
    let mobs = mob_settings.get_mobs_to_spawn(MobCategory::Creature);
    if mobs.is_empty() {
        return;
    }
    let creature_probability = level.get_environment_attribute(environment_attributes::CREATURE_WORLD_GEN_SPAWN_PROBABILITY, source_pos);
    let xo = chunk_pos.min_block_x();
    let zo = chunk_pos.min_block_z();
    while random.next_float() < creature_probability {
        let Some(spawner_data) = mobs.get_random(random) else { continue };
        let entity_type = spawner_data.type_();
        let count = spawner_data.count.sample(random);
        let mut group_spawn_data: Option<SpawnGroupData> = None;
        let mut x = xo + random.next_int_bounded(16);
        let mut z = zo + random.next_int_bounded(16);
        let start_x = x;
        let start_z = z;
        for _ in 0..count {
            let mut success = false;
            let mut attempts = 0;
            while !success && attempts < 4 {
                let pos = get_top_non_colliding_pos(level, entity_type, x, z);
                if entity_type.can_summon() && spawn_placements::is_spawn_position_ok(entity_type, level, pos) {
                    let width = entity_type.get_width() as f64;
                    let fx = mth::clamp(x as f64, xo as f64 + width, xo as f64 + 16.0 - width);
                    let fz = mth::clamp(z as f64, zo as f64 + width, zo as f64 + 16.0 - width);
                    if !level.no_collision(&entity_type.get_spawn_aabb(fx, pos.y as f64, fz))
                        || !spawn_placements::check_spawn_rules(entity_type, level, EntitySpawnReason::ChunkGeneration, BlockPos::containing(fx, pos.y as f64, fz))
                    {
                        attempts += 1;
                        continue;
                    }
                    let Some(entity) = ServerLevel::create_entity(entity_type.get_key()) else {
                        attempts += 1;
                        continue;
                    };
                    let LevelEntity::Mob(mut mob) = entity else {
                        attempts += 1;
                        continue;
                    };
                    let y_rot = random.next_float() * 360.0;
                    mob.entity.snap_to(fx, pos.y as f64, fz, y_rot, 0.0);
                    if mob::check_spawn_rules(&mob, level, EntitySpawnReason::ChunkGeneration) && mob::check_spawn_obstruction(&mob, level) {
                        let difficulty = level.get_current_difficulty_at(mob.entity.block_position());
                        group_spawn_data = mob::finalize_spawn(level, &mut mob, &difficulty, EntitySpawnReason::ChunkGeneration, group_spawn_data);
                        level.add_fresh_entity_with_passengers(LevelEntity::Mob(mob));
                        success = true;
                    }
                }
                x += random.next_int_bounded(5) - random.next_int_bounded(5);
                z += random.next_int_bounded(5) - random.next_int_bounded(5);
                while x < xo || x >= xo + 16 || z < zo || z >= zo + 16 {
                    x = start_x + random.next_int_bounded(5) - random.next_int_bounded(5);
                    z = start_z + random.next_int_bounded(5) - random.next_int_bounded(5);
                }
                attempts += 1;
            }
        }
    }
}

fn get_top_non_colliding_pos(level: &mut (impl WorldGenLevel + ?Sized), entity_type: &EntityType, x: i32, z: i32) -> BlockPos {
    let level_height = level.get_height(spawn_placements::get_heightmap_type(Some(entity_type)), x, z);
    let mut pos = BlockPos::new(x, level_height, z);
    if level.dimension_type().has_ceiling {
        loop {
            pos = pos.relative(Direction::Down);
            if level.block_state(pos).is_air() {
                break;
            }
        }
        loop {
            pos = pos.relative(Direction::Down);
            if !(level.block_state(pos).is_air() && pos.y > level.get_min_y()) {
                break;
            }
        }
    }
    spawn_placements::get_placement_type(entity_type).adjust_spawn_position(level, pos)
}

pub struct SpawnState {
    spawnable_chunk_count: i32,
    mob_category_counts: [i32; 8],
    spawn_potential: PotentialCalculator,
    local_mob_cap_calculator: LocalMobCapCalculator,
    last_checked_pos: Option<BlockPos>,
    last_checked_type: Option<i32>,
    last_charge: f64,
}

impl SpawnState {
    fn new(spawnable_chunk_count: i32, mob_category_counts: [i32; 8], spawn_potential: PotentialCalculator, local_mob_cap_calculator: LocalMobCapCalculator) -> SpawnState {
        SpawnState { spawnable_chunk_count, mob_category_counts, spawn_potential, local_mob_cap_calculator, last_checked_pos: None, last_checked_type: None, last_charge: 0.0 }
    }

    fn can_spawn(&mut self, entity_type: &EntityType, level: &mut ServerLevel, test_pos: BlockPos, _chunk: ChunkPos) -> bool {
        self.last_checked_pos = Some(test_pos);
        self.last_checked_type = Some(entity_type.id);
        let natural_mob_spawns = level.environment_attributes().get_value_at(environment_attributes::NATURAL_MOB_SPAWNS, test_pos);
        match natural_mob_spawns.get_mob_spawn_cost(entity_type.id) {
            None => {
                self.last_charge = 0.0;
                true
            }
            Some(mob_spawn_cost) => {
                let charge = mob_spawn_cost.charge;
                self.last_charge = charge;
                let energy_change = self.spawn_potential.get_potential_energy_change(test_pos, charge);
                energy_change <= mob_spawn_cost.energy_budget
            }
        }
    }

    fn after_spawn(&mut self, level: &mut ServerLevel, mob: EntityId, _chunk: ChunkPos) {
        let entity_type = EntityType::by_id(level.entity(mob).entity_type);
        let pos = level.entity(mob).block_position();
        let charge = if self.last_checked_pos == Some(pos) && self.last_checked_type == Some(entity_type.id) {
            self.last_charge
        } else {
            let natural_mob_spawns = level.environment_attributes().get_value_at(environment_attributes::NATURAL_MOB_SPAWNS, pos);
            natural_mob_spawns.get_mob_spawn_cost(entity_type.id).map_or(0.0, |cost| cost.charge)
        };
        self.spawn_potential.add_charge(pos, charge);
        let category = entity_type.get_category();
        self.mob_category_counts[category as usize] += 1;
        self.local_mob_cap_calculator.add_mob(level, ChunkPos::new(pos.x >> 4, pos.z >> 4), category);
    }

    pub fn get_spawnable_chunk_count(&self) -> i32 {
        self.spawnable_chunk_count
    }

    pub fn get_mob_category_counts(&self) -> &[i32; 8] {
        &self.mob_category_counts
    }

    fn can_spawn_for_category_global(&self, mob_category: MobCategory) -> bool {
        let max_mob_count = mob_category.get_max_instances_per_chunk() * self.spawnable_chunk_count / MAGIC_NUMBER;
        self.mob_category_counts[mob_category as usize] < max_mob_count
    }

    fn can_spawn_for_category_local(&mut self, level: &mut ServerLevel, mob_category: MobCategory, chunk_pos: ChunkPos) -> bool {
        self.local_mob_cap_calculator.can_spawn(level, mob_category, chunk_pos)
    }
}
