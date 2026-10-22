use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::properties::TrialSpawnerState;
use wafer_data::{registry, registry_id};

use super::trial_spawner::TrialSpawner;
use super::trial_spawner_state_data::TrialSpawnerStateData;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::entity::ominous_item_spawner::OminousItemSpawner;
use crate::world::level::BlockGetter;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};

const DELAY_BEFORE_EJECT_AFTER_KILLING_LAST_MOB: f32 = 40.0;
const TIME_BETWEEN_EACH_EJECTION: i32 = 30;

pub fn spinning_mob_speed(state: TrialSpawnerState) -> f64 {
    match state {
        TrialSpawnerState::WaitingForPlayers => 200.0,
        TrialSpawnerState::Active => 1000.0,
        _ => -1.0,
    }
}

pub fn has_spinning_mob(state: TrialSpawnerState) -> bool {
    spinning_mob_speed(state) >= 0.0
}

pub fn tick_and_get_next(state: TrialSpawnerState, spawner_pos: BlockPos, trial_spawner: &mut TrialSpawner, data: &mut TrialSpawnerStateData, level: &mut ServerLevel) -> TrialSpawnerState {
    match state {
        TrialSpawnerState::Inactive => {
            if data.get_or_create_display_entity(trial_spawner, level, spawner_pos, TrialSpawnerState::WaitingForPlayers) {
                TrialSpawnerState::WaitingForPlayers
            } else {
                state
            }
        }
        TrialSpawnerState::WaitingForPlayers => {
            if !trial_spawner.can_spawn_in_level(level) {
                data.reset_statistics();
                state
            } else if !data.has_mob_to_spawn(trial_spawner, level, spawner_pos) {
                TrialSpawnerState::Inactive
            } else {
                data.try_detect_players(level, spawner_pos, trial_spawner);
                if data.detected_players.is_empty() { state } else { TrialSpawnerState::Active }
            }
        }
        TrialSpawnerState::Active => {
            if !trial_spawner.can_spawn_in_level(level) {
                data.reset_statistics();
                TrialSpawnerState::WaitingForPlayers
            } else if !data.has_mob_to_spawn(trial_spawner, level, spawner_pos) {
                TrialSpawnerState::Inactive
            } else {
                let was_ominous = trial_spawner.is_ominous();
                let additional_players = data.count_additional_players(spawner_pos);
                data.try_detect_players(level, spawner_pos, trial_spawner);
                if trial_spawner.is_ominous() {
                    spawn_ominous_ominous_item_spawner(level, spawner_pos, trial_spawner, data);
                }
                let config = if was_ominous { trial_spawner.ominous_config() } else { trial_spawner.normal_config() };
                if data.has_finished_spawning_all_mobs(config, additional_players) {
                    if data.have_all_current_mobs_died() {
                        data.cooldown_ends_at = level.game_time + trial_spawner.get_target_cooldown_length() as i64;
                        data.total_mobs_spawned = 0;
                        data.next_mob_spawns_at = 0;
                        return TrialSpawnerState::WaitingForRewardEjection;
                    }
                } else if data.is_ready_to_spawn_next_mob(level, config, additional_players)
                    && let Some(entity_id) = trial_spawner.spawn_mob(data, level, spawner_pos)
                {
                    let config = if was_ominous { trial_spawner.ominous_config() } else { trial_spawner.normal_config() };
                    data.current_mobs.insert(entity_id);
                    data.total_mobs_spawned += 1;
                    data.next_mob_spawns_at = level.game_time + config.ticks_between_spawn as i64;
                    if let Some(entry) = config.spawn_potentials_definition.get_random(&mut level.random) {
                        data.next_spawn_data = Some(entry.clone());
                        super::trial_spawner::mark_updated(level, spawner_pos);
                    }
                }
                state
            }
        }
        TrialSpawnerState::WaitingForRewardEjection => {
            if data.is_ready_to_open_shutter(level, DELAY_BEFORE_EJECT_AFTER_KILLING_LAST_MOB, trial_spawner.get_target_cooldown_length()) {
                level.play_sound(None, spawner_pos, registry_id(registry::SOUND_EVENT, "minecraft:block.trial_spawner.open_shutter"), SoundSource::Blocks, 1.0, 1.0);
                TrialSpawnerState::EjectingReward
            } else {
                state
            }
        }
        TrialSpawnerState::EjectingReward => {
            if !data.is_ready_to_eject_items(level, TIME_BETWEEN_EACH_EJECTION as f32, trial_spawner.get_target_cooldown_length()) {
                state
            } else if data.detected_players.is_empty() {
                level.play_sound(None, spawner_pos, registry_id(registry::SOUND_EVENT, "minecraft:block.trial_spawner.close_shutter"), SoundSource::Blocks, 1.0, 1.0);
                data.ejecting_loot_table = None;
                TrialSpawnerState::Cooldown
            } else {
                if data.ejecting_loot_table.is_none() {
                    data.ejecting_loot_table = trial_spawner.active_config().loot_tables_to_eject.get_random(&mut level.random).cloned();
                }
                if let Some(loot_table) = &data.ejecting_loot_table {
                    trial_spawner.eject_reward(level, spawner_pos, loot_table);
                }
                if let Some(first) = data.detected_players.first() {
                    data.detected_players.remove(&first);
                }
                state
            }
        }
        TrialSpawnerState::Cooldown => {
            data.try_detect_players(level, spawner_pos, trial_spawner);
            if !data.detected_players.is_empty() {
                data.total_mobs_spawned = 0;
                data.next_mob_spawns_at = 0;
                TrialSpawnerState::Active
            } else if data.is_cooldown_finished(level) {
                trial_spawner.remove_ominous(level, spawner_pos);
                data.reset();
                TrialSpawnerState::WaitingForPlayers
            } else {
                state
            }
        }
    }
}

fn spawn_ominous_ominous_item_spawner(level: &mut ServerLevel, trial_spawner_pos: BlockPos, trial_spawner: &TrialSpawner, data: &mut TrialSpawnerStateData) {
    let config = trial_spawner.active_config();
    let item_to_dispense = data.get_dispensing_items(level, config, trial_spawner_pos).get_random(&mut level.random).cloned().unwrap_or_else(crate::world::item::ItemStack::empty);
    if item_to_dispense.is_empty() {
        return;
    }
    if time_to_spawn_item_spawner(level, data)
        && let Some(pos) = calculate_position_to_spawn_spawner(level, trial_spawner_pos, trial_spawner, data)
    {
        let mut item_spawner = OminousItemSpawner::create(level, item_to_dispense);
        let (y_rot, x_rot) = (item_spawner.entity.y_rot, item_spawner.entity.x_rot);
        item_spawner.entity.snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
        level.add_fresh_entity(crate::world::entity::LevelEntity::OminousItemSpawner(Box::new(item_spawner)));
        let pitch = (level.random.next_float() - level.random.next_float()) * 0.2 + 1.0;
        level.play_sound(None, BlockPos::containing_vec(pos), registry_id(registry::SOUND_EVENT, "minecraft:block.trial_spawner.spawn_item_begin"), SoundSource::Blocks, 1.0, pitch);
        data.cooldown_ends_at = level.game_time + trial_spawner.ominous_config().ticks_between_item_spawners();
    }
}

fn calculate_position_to_spawn_spawner(level: &mut ServerLevel, trial_spawner_pos: BlockPos, trial_spawner: &TrialSpawner, data: &TrialSpawnerStateData) -> Option<Vec3> {
    let range_sqr = (trial_spawner.get_required_player_range() * trial_spawner.get_required_player_range()) as f64;
    let center = Vec3::at_center_of(trial_spawner_pos);
    let nearby_players: Vec<EntityId> = data
        .detected_players
        .iter()
        .filter_map(|uuid| level.get_player_by_uuid(uuid))
        .filter(|&player| {
            let game_type = level.player(player).game_mode.game_type;
            !game_type.is_creative() && !level.is_spectator(player) && level.is_alive(player) && level.entity(player).distance_to_sqr_xyz(center.x, center.y, center.z) <= range_sqr
        })
        .collect();
    if nearby_players.is_empty() {
        return None;
    }
    let entity = select_entity_to_spawn_item_above(&nearby_players, data, trial_spawner, trial_spawner_pos, level)?;
    calculate_position_above(entity, level)
}

fn calculate_position_above(entity_to_spawn_item_above: EntityId, level: &mut ServerLevel) -> Option<Vec3> {
    let entity_pos = level.entity(entity_to_spawn_item_above).position();
    let bb_height = level.entity(entity_to_spawn_item_above).dimensions.height;
    let try_spawn_pos = entity_pos.relative(Direction::Up, (bb_height + 2.0 + level.random.next_int_bounded(4) as f32) as f64);
    let hit_result = level.clip(&ClipContext { from: entity_pos, to: try_spawn_pos, block: ClipBlock::Visual, fluid: ClipFluid::None, is_player: false });
    let down = Vec3::at_center_of(hit_result.hit.block_pos).relative(Direction::Down, 1.0);
    let block_pos_down = BlockPos::containing_vec(down);
    if !level.block_state(block_pos_down).get_collision_shape(block_pos_down).is_empty() { None } else { Some(down) }
}

fn select_entity_to_spawn_item_above(nearby_players: &[EntityId], data: &TrialSpawnerStateData, trial_spawner: &TrialSpawner, spawner_pos: BlockPos, level: &mut ServerLevel) -> Option<EntityId> {
    let range_sqr = (trial_spawner.get_required_player_range() * trial_spawner.get_required_player_range()) as f64;
    let center = Vec3::at_center_of(spawner_pos);
    let eligible_entities: Vec<EntityId> = if level.random.next_boolean() {
        data.current_mobs
            .iter()
            .filter_map(|uuid| level.entity_manager.visible_entity_storage.get(uuid).filter(|&id| level.entity_ref(id).is_some_and(|e| !e.is_removed())))
            .filter(|&target| level.is_alive(target) && level.entity(target).distance_to_sqr_xyz(center.x, center.y, center.z) <= range_sqr)
            .collect()
    } else {
        nearby_players.to_vec()
    };
    match eligible_entities.len() {
        0 => None,
        1 => Some(eligible_entities[0]),
        len => Some(eligible_entities[level.random.next_int_bounded(len as i32) as usize]),
    }
}

fn time_to_spawn_item_spawner(level: &ServerLevel, data: &TrialSpawnerStateData) -> bool {
    level.game_time >= data.cooldown_ends_at
}
