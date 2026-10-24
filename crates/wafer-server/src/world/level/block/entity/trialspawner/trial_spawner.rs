use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::BlockState;
use wafer_data::game_rule;
use wafer_data::properties::{OMINOUS, TRIAL_SPAWNER_STATE, TrialSpawnerState};
use wafer_protocol::nbt::Nbt;

use super::player_detector::{EntitySelector, PlayerDetector};
use super::trial_spawner_config::{self, TrialSpawnerConfig};
use super::trial_spawner_state;
use super::trial_spawner_state_data::TrialSpawnerStateData;
use crate::core::dispenser::default_dispense_item_behavior;
use crate::core::holder::Holder;
use crate::server::level::{ServerLevel, events};
use crate::util::codec::{self, DataResult};
use crate::util::problem_reporter::PathElement;
use crate::world::level::storage::tag_value_input::ScopedCollector;
use crate::world::difficulty::Difficulty;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::{LevelEntity, mob, spawn_placements};
use crate::world::level::BlockGetter;
use crate::world::level::base_spawner;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::gameevent::game_event;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::spawn_data::SpawnData;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::update_flags::UPDATE_ALL;

pub const DETECT_PLAYER_SPAWN_BUFFER: i64 = 40;
const TRIAL_SPAWNER_LOGGER: &str = "net.minecraft.world.level.block.entity.trialspawner.TrialSpawner";
const DEFAULT_TARGET_COOLDOWN_LENGTH: i32 = 36000;
const DEFAULT_PLAYER_SCAN_RANGE: i32 = 14;
const MAX_MOB_TRACKING_DISTANCE: i32 = 47;
const MAX_MOB_TRACKING_DISTANCE_SQR: f64 = (MAX_MOB_TRACKING_DISTANCE * MAX_MOB_TRACKING_DISTANCE) as f64;

#[derive(Clone)]
pub struct TrialSpawner {
    data: TrialSpawnerStateData,
    config: FullConfig,
    player_detector: PlayerDetector,
    entity_selector: EntitySelector,
    override_peaceful_and_mob_spawn_rule: bool,
    is_ominous: bool,
}

impl Default for TrialSpawner {
    fn default() -> TrialSpawner {
        TrialSpawner {
            data: TrialSpawnerStateData::default(),
            config: FullConfig::default(),
            player_detector: PlayerDetector::NoCreativePlayers,
            entity_selector: EntitySelector::SelectFromLevel,
            override_peaceful_and_mob_spawn_rule: false,
            is_ominous: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlameParticle {
    Normal,
    Ominous,
}

impl FlameParticle {
    pub fn encode(self) -> i32 {
        self as i32
    }
}

impl TrialSpawner {
    pub fn active_config(&self) -> &TrialSpawnerConfig {
        if self.is_ominous { self.config.ominous.value() } else { self.config.normal.value() }
    }

    pub fn normal_config(&self) -> &TrialSpawnerConfig {
        self.config.normal.value()
    }

    pub fn ominous_config(&self) -> &TrialSpawnerConfig {
        self.config.ominous.value()
    }

    pub fn load(&mut self, input: &Nbt) {
        self.data.apply(input);
        self.config = FullConfig::decode(input).unwrap_or_default();
    }

    pub fn store(&self, output: &mut Vec<(String, Nbt)>) {
        self.data.pack(output);
        self.config.encode(output);
    }

    pub fn apply_ominous(&mut self, data: &mut TrialSpawnerStateData, level: &mut ServerLevel, spawner_pos: BlockPos) {
        let state = level.block_state(spawner_pos);
        level.set_block(spawner_pos, state.set(OMINOUS, true), UPDATE_ALL);
        level.level_event(None, events::PARTICLES_TRIAL_SPAWNER_BECOME_OMINOUS, spawner_pos, 1);
        self.is_ominous = true;
        data.reset_after_becoming_ominous(self, level, spawner_pos);
    }

    pub fn remove_ominous(&mut self, level: &mut ServerLevel, spawner_pos: BlockPos) {
        let state = level.block_state(spawner_pos);
        level.set_block(spawner_pos, state.set(OMINOUS, false), UPDATE_ALL);
        self.is_ominous = false;
    }

    pub fn is_ominous(&self) -> bool {
        self.is_ominous
    }

    pub fn get_target_cooldown_length(&self) -> i32 {
        self.config.target_cooldown_length
    }

    pub fn get_required_player_range(&self) -> i32 {
        self.config.required_player_range
    }

    pub fn get_state_data(&self) -> &TrialSpawnerStateData {
        &self.data
    }

    pub fn get_player_detector(&self) -> PlayerDetector {
        self.player_detector
    }

    pub fn get_entity_selector(&self) -> EntitySelector {
        self.entity_selector
    }

    pub fn can_spawn_in_level(&self, level: &ServerLevel) -> bool {
        if !level.game_rules.get_bool(game_rule::SPAWNER_BLOCKS_WORK) {
            false
        } else if self.override_peaceful_and_mob_spawn_rule {
            true
        } else if level.difficulty == Difficulty::Peaceful {
            false
        } else {
            level.game_rules.get_bool(game_rule::SPAWN_MOBS)
        }
    }

    pub fn spawn_mob(&self, data: &mut TrialSpawnerStateData, level: &mut ServerLevel, spawner_pos: BlockPos) -> Option<u128> {
        let next_spawn_data = data.get_or_create_next_spawn_data(self, level, spawner_pos);
        let input = next_spawn_data.get_entity_to_spawn();
        let _collector = ScopedCollector::new(PathElement::Text(format!("spawner@BlockPos{{x={}, y={}, z={}}}", spawner_pos.x, spawner_pos.y, spawner_pos.z)), TRIAL_SPAWNER_LOGGER);
        let entity_type = base_spawner::entity_type_by(input)?;
        let spawn_pos = match base_spawner::read_spawn_pos(input) {
            Some(spawn_pos) => spawn_pos,
            None => {
                let spawn_range = self.active_config().spawn_range as f64;
                let random = &mut level.random;
                let x = spawner_pos.x as f64 + (random.next_double() - random.next_double()) * spawn_range + 0.5;
                let y = (spawner_pos.y + random.next_int_bounded(3) - 1) as f64;
                let z = spawner_pos.z as f64 + (random.next_double() - random.next_double()) * spawn_range + 0.5;
                Vec3::new(x, y, z)
            }
        };
        if !ServerLevelAccessor::no_collision(level, &entity_type.get_spawn_aabb(spawn_pos.x, spawn_pos.y, spawn_pos.z)) {
            return None;
        }
        if !in_line_of_sight(level, Vec3::at_center_of(spawner_pos), spawn_pos) {
            return None;
        }
        let spawn_block_pos = BlockPos::containing_vec(spawn_pos);
        if !spawn_placements::check_spawn_rules(entity_type, level, EntitySpawnReason::TrialSpawner, spawn_block_pos) {
            return None;
        }
        if let Some(custom_spawn_rules) = next_spawn_data.get_custom_spawn_rules()
            && !custom_spawn_rules.is_valid_position(spawn_block_pos, level)
        {
            return None;
        }
        let mut loaded = level.load_entity_recursive_for_spawn(input, &mut |_| {});
        for entity in &mut loaded {
            let y_rot = level.random.next_float() * 360.0;
            entity.entity_mut().snap_to(spawn_pos.x, spawn_pos.y, spawn_pos.z, y_rot, 0.0);
        }
        if loaded.is_empty() {
            return None;
        }
        if let LevelEntity::Mob(mob) = &loaded[0] {
            if !mob::check_spawn_obstruction(mob, level) {
                return None;
            }
            let has_no_configuration = matches!(input, Nbt::Compound(fields) if fields.len() == 1) && input.get("id").and_then(Nbt::as_str).is_some();
            if has_no_configuration {
                let entity = loaded.remove(0);
                let difficulty = level.current_difficulty_at(entity.entity().block_position());
                let (entity, _) = level.finalize_spawn_detached(entity, &difficulty, EntitySpawnReason::TrialSpawner, None);
                loaded.insert(0, entity);
            }
            if let LevelEntity::Mob(mob) = &mut loaded[0] {
                mob.persistence_required = true;
            }
            if let Some(equipment) = next_spawn_data.get_equipment() {
                let entity = loaded.remove(0);
                loaded.insert(0, level.mob_equip_detached(entity, equipment));
            }
        }
        let (id, uuid) = (loaded[0].entity().id, loaded[0].entity().uuid);
        if !level.try_add_fresh_entity_with_passengers(loaded) {
            return None;
        }
        let flame_particle = if self.is_ominous { FlameParticle::Ominous } else { FlameParticle::Normal };
        level.level_event(None, events::PARTICLES_TRIAL_SPAWNER_SPAWN, spawner_pos, flame_particle.encode());
        level.level_event(None, events::PARTICLES_TRIAL_SPAWNER_SPAWN_MOB_AT, spawn_block_pos, flame_particle.encode());
        level.game_event_at(Some(id), game_event::ENTITY_PLACE, spawn_block_pos);
        Some(uuid)
    }

    pub fn eject_reward(&self, level: &mut ServerLevel, pos: BlockPos, ejecting_loot_table: &str) {
        let loot_table = LootTable::get(ejecting_loot_table);
        let params = LootParams::new(level);
        let loot_drops = loot_table.get_random_items(params);
        if !loot_drops.is_empty() {
            for item in loot_drops {
                default_dispense_item_behavior::spawn_item(level, item, 2, Direction::Up, Vec3::at_bottom_center_of(pos).relative(Direction::Up, 1.2));
            }
            level.level_event(None, events::ANIMATION_TRIAL_SPAWNER_EJECT_ITEM, pos, 0);
        }
    }

    pub fn tick_server(&mut self, level: &mut ServerLevel, spawner_pos: BlockPos, is_ominous: bool) {
        self.is_ominous = is_ominous;
        let current_state = get_state(level, spawner_pos);
        let mut data = std::mem::take(&mut self.data);
        let untracked: Vec<u128> = data.current_mobs.iter().filter(|&id| should_mob_be_untracked(level, spawner_pos, id)).collect();
        if !untracked.is_empty() {
            for id in &untracked {
                data.current_mobs.remove(id);
            }
            data.next_mob_spawns_at = level.game_time + self.active_config().ticks_between_spawn as i64;
        }
        let next_state = trial_spawner_state::tick_and_get_next(current_state, spawner_pos, self, &mut data, level);
        self.data = data;
        if next_state != current_state {
            set_state(level, spawner_pos, next_state);
        }
    }

    pub fn override_entity_to_spawn(&mut self, level: &mut ServerLevel, pos: BlockPos, spawn_data: SpawnData) {
        self.data.reset();
        self.config = self.config.override_entity(spawn_data);
        set_state(level, pos, TrialSpawnerState::Inactive);
    }
}

fn should_mob_be_untracked(level: &ServerLevel, spawner_pos: BlockPos, id: u128) -> bool {
    match level.entity_manager.visible_entity_storage.get(id) {
        Some(entity) => !level.is_alive(entity) || level.entity(entity).block_position().dist_sqr(spawner_pos) > MAX_MOB_TRACKING_DISTANCE_SQR,
        None => true,
    }
}

pub fn in_line_of_sight(level: &mut ServerLevel, origin: Vec3, dest: Vec3) -> bool {
    let hit_result = level.clip(&ClipContext { from: dest, to: origin, block: ClipBlock::Visual, fluid: ClipFluid::None, collision_context: CollisionContext::EMPTY });
    hit_result.hit.block_pos == BlockPos::containing_vec(origin) || hit_result.miss
}

pub fn get_state(level: &ServerLevel, pos: BlockPos) -> TrialSpawnerState {
    let state = level.chunks.block_entity_ref(pos).map_or(wafer_data::blocks::AIR.default_state(), |entity| entity.block_state());
    state.try_get(TRIAL_SPAWNER_STATE).unwrap_or(TrialSpawnerState::Inactive)
}

pub fn set_state(level: &mut ServerLevel, pos: BlockPos, state: TrialSpawnerState) {
    level.block_entity_set_changed(pos);
    let Some(block_state) = level.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return };
    level.set_block(pos, block_state.set(TRIAL_SPAWNER_STATE, state), UPDATE_ALL);
}

pub fn mark_updated(level: &mut ServerLevel, pos: BlockPos) {
    level.block_entity_set_changed(pos);
    let Some(block_state) = level.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return };
    level.send_block_updated(pos, block_state, block_state);
}

pub fn block_state_is_ominous(state: BlockState) -> bool {
    state.try_get(OMINOUS).unwrap_or(false)
}

#[derive(Clone)]
pub struct FullConfig {
    pub normal: Holder<TrialSpawnerConfig>,
    pub ominous: Holder<TrialSpawnerConfig>,
    pub target_cooldown_length: i32,
    pub required_player_range: i32,
}

impl Default for FullConfig {
    fn default() -> FullConfig {
        FullConfig {
            normal: Holder::direct(trial_spawner_config::DEFAULT.clone()),
            ominous: Holder::direct(trial_spawner_config::DEFAULT.clone()),
            target_cooldown_length: DEFAULT_TARGET_COOLDOWN_LENGTH,
            required_player_range: DEFAULT_PLAYER_SCAN_RANGE,
        }
    }
}

fn is_default_holder(holder: &Holder<TrialSpawnerConfig>) -> bool {
    holder.unwrap_key().is_none() && *holder.value() == *trial_spawner_config::DEFAULT
}

impl FullConfig {
    pub fn decode(input: &Nbt) -> DataResult<FullConfig> {
        let default = || Holder::direct(trial_spawner_config::DEFAULT.clone());
        Ok(FullConfig {
            normal: match codec::optional_field(input, "normal_config") {
                Some(tag) => TrialSpawnerConfig::decode(tag)?,
                None => default(),
            },
            ominous: match codec::optional_field(input, "ominous_config") {
                Some(tag) => TrialSpawnerConfig::decode(tag)?,
                None => default(),
            },
            target_cooldown_length: codec::decode_or(input, "target_cooldown_length", DEFAULT_TARGET_COOLDOWN_LENGTH, codec::int)?,
            required_player_range: codec::decode_or(input, "required_player_range", DEFAULT_PLAYER_SCAN_RANGE, codec::int)?,
        })
    }

    pub fn encode(&self, output: &mut Vec<(String, Nbt)>) {
        if !is_default_holder(&self.normal) {
            output.push(("normal_config".into(), TrialSpawnerConfig::encode(&self.normal)));
        }
        if !is_default_holder(&self.ominous) {
            output.push(("ominous_config".into(), TrialSpawnerConfig::encode(&self.ominous)));
        }
        if self.target_cooldown_length != DEFAULT_TARGET_COOLDOWN_LENGTH {
            output.push(("target_cooldown_length".into(), Nbt::Int(self.target_cooldown_length)));
        }
        if self.required_player_range != DEFAULT_PLAYER_SCAN_RANGE {
            output.push(("required_player_range".into(), Nbt::Int(self.required_player_range)));
        }
    }

    pub fn override_entity(&self, spawn_data: SpawnData) -> FullConfig {
        FullConfig {
            normal: Holder::direct(self.normal.value().with_spawning(spawn_data.clone())),
            ominous: Holder::direct(self.ominous.value().with_spawning(spawn_data)),
            target_cooldown_length: self.target_cooldown_length,
            required_player_range: self.required_player_range,
        }
    }
}
