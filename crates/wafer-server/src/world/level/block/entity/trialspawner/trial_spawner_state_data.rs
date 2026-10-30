use wafer_core::{BlockPos, mth};
use wafer_data::properties::TrialSpawnerState;
use wafer_data::{MobEffect, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::trial_spawner::{self, FlameParticle, TrialSpawner};
use super::trial_spawner_config::TrialSpawnerConfig;
use super::trial_spawner_state;
use crate::server::level::{ServerLevel, events};
use crate::util::codec;
use crate::util::java_collections::JavaObjectHashSet;
use crate::util::random::WeightedList;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::{EntityId, RemovalReason};
use crate::world::entity::entity_save::{LOAD_ENTITY_RECURSIVE_TAG, block_entity_tick_site, uuid_from_nbt};
use crate::world::item::ItemStack;
use crate::world::level::spawn_data::SpawnData;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::storage::uuid_ints;

const GET_OR_CREATE_DISPLAY_ENTITY: &str = "net.minecraft.world.level.block.entity.trialspawner.TrialSpawnerStateData.getOrCreateDisplayEntity(TrialSpawnerStateData.java:274)";
const TAG_SPAWN_DATA: &str = "spawn_data";
const TAG_NEXT_MOB_SPAWNS_AT: &str = "next_mob_spawns_at";
const DELAY_BETWEEN_PLAYER_SCANS: i64 = 20;
const TRIAL_OMEN_PER_BAD_OMEN_LEVEL: i32 = 18000;

#[derive(Clone, Default)]
pub struct TrialSpawnerStateData {
    pub detected_players: JavaObjectHashSet<u128>,
    pub current_mobs: JavaObjectHashSet<u128>,
    pub cooldown_ends_at: i64,
    pub next_mob_spawns_at: i64,
    pub total_mobs_spawned: i32,
    pub next_spawn_data: Option<SpawnData>,
    pub ejecting_loot_table: Option<String>,
    display_entity: bool,
    dispensing: Option<WeightedList<ItemStack>>,
}

fn uuid_set(tag: Option<&Nbt>) -> Option<Vec<u128>> {
    match tag? {
        Nbt::List(list) => list.iter().map(uuid_from_nbt).collect(),
        _ => None,
    }
}

fn encode_uuid_set(set: &JavaObjectHashSet<u128>) -> Nbt {
    Nbt::List(set.iter().map(uuid_ints).collect())
}

impl TrialSpawnerStateData {
    pub fn pack(&self, output: &mut Vec<(String, Nbt)>) {
        if !self.detected_players.is_empty() {
            output.push(("registered_players".into(), encode_uuid_set(&self.detected_players)));
        }
        if !self.current_mobs.is_empty() {
            output.push(("current_mobs".into(), encode_uuid_set(&self.current_mobs)));
        }
        if self.cooldown_ends_at != 0 {
            output.push(("cooldown_ends_at".into(), Nbt::Long(self.cooldown_ends_at)));
        }
        if self.next_mob_spawns_at != 0 {
            output.push((TAG_NEXT_MOB_SPAWNS_AT.into(), Nbt::Long(self.next_mob_spawns_at)));
        }
        if self.total_mobs_spawned != 0 {
            output.push(("total_mobs_spawned".into(), Nbt::Int(self.total_mobs_spawned)));
        }
        if let Some(next_spawn_data) = &self.next_spawn_data {
            output.push((TAG_SPAWN_DATA.into(), next_spawn_data.encode()));
        }
        if let Some(ejecting_loot_table) = &self.ejecting_loot_table {
            output.push(("ejecting_loot_table".into(), Nbt::String(ejecting_loot_table.clone())));
        }
    }

    pub fn apply(&mut self, input: &Nbt) {
        if codec::map(input).is_err() {
            return;
        }
        self.detected_players.clear();
        for uuid in uuid_set(input.get("registered_players")).unwrap_or_default() {
            self.detected_players.insert(uuid);
        }
        self.current_mobs.clear();
        for uuid in uuid_set(input.get("current_mobs")).unwrap_or_default() {
            self.current_mobs.insert(uuid);
        }
        self.cooldown_ends_at = input.get("cooldown_ends_at").and_then(|tag| codec::long(tag).ok()).unwrap_or(0);
        self.next_mob_spawns_at = input.get(TAG_NEXT_MOB_SPAWNS_AT).and_then(|tag| codec::long(tag).ok()).unwrap_or(0);
        self.total_mobs_spawned = input.get("total_mobs_spawned").and_then(|tag| codec::int(tag).ok()).filter(|&value| value >= 0).unwrap_or(0);
        self.next_spawn_data = input.get(TAG_SPAWN_DATA).and_then(|tag| SpawnData::decode(tag).ok());
        self.ejecting_loot_table = input.get("ejecting_loot_table").and_then(|tag| codec::identifier(tag).ok());
    }

    pub fn reset(&mut self) {
        self.current_mobs.clear();
        self.next_spawn_data = None;
        self.reset_statistics();
    }

    pub fn reset_statistics(&mut self) {
        self.detected_players.clear();
        self.total_mobs_spawned = 0;
        self.next_mob_spawns_at = 0;
        self.cooldown_ends_at = 0;
    }

    pub fn has_mob_to_spawn(&mut self, trial_spawner: &TrialSpawner, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let has_next_mob_to_spawn = self.get_or_create_next_spawn_data(trial_spawner, level, pos).get_entity_to_spawn().get("id").and_then(Nbt::as_str).is_some();
        has_next_mob_to_spawn || !trial_spawner.active_config().spawn_potentials_definition.is_empty()
    }

    pub fn has_finished_spawning_all_mobs(&self, config: &TrialSpawnerConfig, additional_players: i32) -> bool {
        self.total_mobs_spawned >= config.calculate_target_total_mobs(additional_players)
    }

    pub fn have_all_current_mobs_died(&self) -> bool {
        self.current_mobs.is_empty()
    }

    pub fn is_ready_to_spawn_next_mob(&self, level: &ServerLevel, config: &TrialSpawnerConfig, additional_players: i32) -> bool {
        level.game_time >= self.next_mob_spawns_at && (self.current_mobs.len() as i32) < config.calculate_target_simultaneous_mobs(additional_players)
    }

    pub fn count_additional_players(&self, _pos: BlockPos) -> i32 {
        0.max(self.detected_players.len() as i32 - 1)
    }

    pub fn try_detect_players(&mut self, level: &mut ServerLevel, pos: BlockPos, trial_spawner: &mut TrialSpawner) {
        let is_throttled = (pos.as_long().wrapping_add(level.game_time)) % DELAY_BETWEEN_PLAYER_SCANS != 0;
        if is_throttled {
            return;
        }
        let state = trial_spawner::get_state(level, pos);
        if state == TrialSpawnerState::Cooldown && trial_spawner.is_ominous() {
            return;
        }
        let range = trial_spawner.get_required_player_range() as f64;
        let in_line_of_sight_players = trial_spawner.get_player_detector().detect(level, trial_spawner.get_entity_selector(), pos, range, true);
        let became_ominous = if !trial_spawner.is_ominous() && !in_line_of_sight_players.is_empty() {
            let player_with_ominous_effect = find_player_with_ominous_effect(level, &in_line_of_sight_players);
            if let Some((player, effect)) = player_with_ominous_effect {
                if effect == mob_effects::BAD_OMEN {
                    transform_bad_omen_into_trial_omen(level, player);
                }
                let eye = level.entity(player).eye_position();
                level.level_event(None, events::PARTICLES_TRIAL_SPAWNER_BECOME_OMINOUS, BlockPos::containing_vec(eye), 0);
                trial_spawner.apply_ominous(self, level, pos);
            }
            player_with_ominous_effect.is_some()
        } else {
            false
        };
        if trial_spawner::get_state(level, pos) != TrialSpawnerState::Cooldown || became_ominous {
            let is_searching_for_first_player = self.detected_players.is_empty();
            let found_players =
                if is_searching_for_first_player { in_line_of_sight_players } else { trial_spawner.get_player_detector().detect(level, trial_spawner.get_entity_selector(), pos, range, false) };
            let mut added = false;
            for uuid in found_players {
                added |= self.detected_players.insert(uuid);
            }
            if added {
                self.next_mob_spawns_at = (level.game_time + trial_spawner::DETECT_PLAYER_SPAWN_BUFFER).max(self.next_mob_spawns_at);
                if !became_ominous {
                    let event = if trial_spawner.is_ominous() { events::PARTICLES_TRIAL_SPAWNER_DETECT_PLAYER_OMINOUS } else { events::PARTICLES_TRIAL_SPAWNER_DETECT_PLAYER };
                    level.level_event(None, event, pos, self.detected_players.len() as i32);
                }
            }
        }
    }

    pub fn reset_after_becoming_ominous(&mut self, trial_spawner: &TrialSpawner, level: &mut ServerLevel, pos: BlockPos) {
        let mobs: Vec<u128> = self.current_mobs.iter().collect();
        for uuid in mobs {
            if let Some(entity) = level.entity_manager.visible_entity_storage.get(uuid).filter(|&id| level.entity_ref(id).is_some_and(|e| !e.is_removed())) {
                let block_position = level.entity(entity).block_position();
                level.level_event(None, events::PARTICLES_TRIAL_SPAWNER_SPAWN_MOB_AT, block_position, FlameParticle::Normal.encode());
                if level.mob(entity).is_some() {
                    level.drop_preserved_equipment(entity);
                }
                level.remove(entity, RemovalReason::Discarded);
            }
        }
        if !trial_spawner.ominous_config().spawn_potentials_definition.is_empty() {
            self.next_spawn_data = None;
        }
        self.total_mobs_spawned = 0;
        self.current_mobs.clear();
        self.next_mob_spawns_at = level.game_time + trial_spawner.ominous_config().ticks_between_spawn as i64;
        trial_spawner::mark_updated(level, pos);
        self.cooldown_ends_at = level.game_time + trial_spawner.ominous_config().ticks_between_item_spawners();
    }

    pub fn is_ready_to_open_shutter(&self, level: &ServerLevel, delay_before_open: f32, target_cooldown_length: i32) -> bool {
        let cooldown_started_at = self.cooldown_ends_at - target_cooldown_length as i64;
        level.game_time as f32 >= cooldown_started_at as f32 + delay_before_open
    }

    pub fn is_ready_to_eject_items(&self, level: &ServerLevel, time_between_ejections: f32, target_cooldown_length: i32) -> bool {
        let cooldown_started_at = self.cooldown_ends_at - target_cooldown_length as i64;
        (level.game_time - cooldown_started_at) as f32 % time_between_ejections == 0.0
    }

    pub fn is_cooldown_finished(&self, level: &ServerLevel) -> bool {
        level.game_time >= self.cooldown_ends_at
    }

    pub fn get_or_create_next_spawn_data(&mut self, trial_spawner: &TrialSpawner, level: &mut ServerLevel, pos: BlockPos) -> &SpawnData {
        if self.next_spawn_data.is_none() {
            let spawn_potentials = &trial_spawner.active_config().spawn_potentials_definition;
            let selected = if spawn_potentials.is_empty() { None } else { spawn_potentials.get_random(&mut level.random).cloned() };
            self.next_spawn_data = Some(selected.unwrap_or_default());
            trial_spawner::mark_updated(level, pos);
        }
        self.next_spawn_data.as_ref().expect("the next spawn data")
    }

    pub fn get_or_create_display_entity(&mut self, trial_spawner: &TrialSpawner, level: &mut ServerLevel, pos: BlockPos, state: TrialSpawnerState) -> bool {
        if !trial_spawner_state::has_spinning_mob(state) {
            return false;
        }
        if !self.display_entity {
            let entity_to_spawn = self.get_or_create_next_spawn_data(trial_spawner, level, pos).get_entity_to_spawn();
            if entity_to_spawn.get("id").and_then(Nbt::as_str).is_some() {
                let entity_to_spawn = entity_to_spawn.clone();
                let mut caller_frames = vec![LOAD_ENTITY_RECURSIVE_TAG, GET_OR_CREATE_DISPLAY_ENTITY];
                caller_frames.extend(trial_spawner::TICK_SERVER_SITE);
                let site = block_entity_tick_site(&caller_frames);
                self.display_entity = !level.load_entity_recursive(&entity_to_spawn, &site, &mut |_| {}).is_empty();
            }
        }
        self.display_entity
    }

    pub fn get_update_tag(&self, state: TrialSpawnerState) -> Nbt {
        let mut tag = Vec::new();
        if state == TrialSpawnerState::Active {
            tag.push((TAG_NEXT_MOB_SPAWNS_AT.into(), Nbt::Long(self.next_mob_spawns_at)));
        }
        if let Some(spawn_data) = &self.next_spawn_data {
            tag.push((TAG_SPAWN_DATA.into(), spawn_data.encode()));
        }
        Nbt::Compound(tag.into())
    }

    pub fn get_dispensing_items(&mut self, level: &mut ServerLevel, config: &TrialSpawnerConfig, pos: BlockPos) -> &WeightedList<ItemStack> {
        if self.dispensing.is_none() {
            let loot_table = LootTable::get(&config.items_to_drop_when_ominous);
            let simple_positional_seed = low_resolution_position(level, pos);
            let params = LootParams::new(level);
            let loot_drops = loot_table.get_random_items_seeded(params, simple_positional_seed);
            if loot_drops.is_empty() {
                static EMPTY: WeightedList<ItemStack> = WeightedList::empty();
                return &EMPTY;
            }
            self.dispensing = Some(WeightedList::new(loot_drops.iter().map(|drop| (drop.copy_with_count(1), drop.count)).collect()));
        }
        self.dispensing.as_ref().expect("the dispensing items")
    }
}

fn find_player_with_ominous_effect(level: &ServerLevel, in_line_of_sight_players: &[u128]) -> Option<(EntityId, MobEffect)> {
    let mut player_with_bad_omen = None;
    for &player_uuid in in_line_of_sight_players {
        if let Some(player) = level.get_player_by_uuid(player_uuid) {
            if level.has_effect(player, mob_effects::TRIAL_OMEN) {
                return Some((player, mob_effects::TRIAL_OMEN));
            }
            if level.has_effect(player, mob_effects::BAD_OMEN) {
                player_with_bad_omen = Some(player);
            }
        }
    }
    player_with_bad_omen.map(|player| (player, mob_effects::BAD_OMEN))
}

fn transform_bad_omen_into_trial_omen(level: &mut ServerLevel, player: EntityId) {
    if let Some(bad_omen) = level.living(player).effect(mob_effects::BAD_OMEN) {
        let amplifier = bad_omen.amplifier + 1;
        let duration = TRIAL_OMEN_PER_BAD_OMEN_LEVEL * amplifier;
        level.remove_effect(player, mob_effects::BAD_OMEN);
        level.add_effect(player, MobEffectInstance::new(mob_effects::TRIAL_OMEN, duration, 0), None);
    }
}

fn low_resolution_position(level: &ServerLevel, pos: BlockPos) -> i64 {
    let low_resolution_position = BlockPos::new(mth::floor((pos.x as f32 / 30.0) as f64), mth::floor((pos.y as f32 / 20.0) as f64), mth::floor((pos.z as f32 / 30.0) as f64));
    level.seed.wrapping_add(low_resolution_position.as_long())
}
