use wafer_core::BlockPos;
use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_data::game_rule;

use super::wandering_trader::WanderingTrader;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::spawn_placements;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

const DEFAULT_TICK_DELAY: i32 = 1200;
pub const DEFAULT_SPAWN_DELAY: i32 = 24000;
pub const MIN_SPAWN_CHANCE: i32 = 25;
const MAX_SPAWN_CHANCE: i32 = 75;
const SPAWN_CHANCE_INCREASE: i32 = 25;
const SPAWN_ONE_IN_X_CHANCE: i32 = 10;
const NUMBER_OF_SPAWN_ATTEMPTS: i32 = 10;

pub struct WanderingTraderSpawner {
    random: LegacyRandomSource,
    tick_delay: i32,
}

impl Default for WanderingTraderSpawner {
    fn default() -> WanderingTraderSpawner {
        WanderingTraderSpawner { random: LegacyRandomSource::create(), tick_delay: DEFAULT_TICK_DELAY }
    }
}

impl WanderingTraderSpawner {
    pub fn tick(&mut self, level: &mut ServerLevel, _spawn_enemies: bool) {
        if !level.game_rules.get_bool(game_rule::SPAWN_WANDERING_TRADERS) {
            return;
        }
        self.tick_delay -= 1;
        if self.tick_delay > 0 {
            return;
        }
        self.tick_delay = DEFAULT_TICK_DELAY;
        let storage = level.server_data_storage.clone();
        let mut storage = storage.borrow_mut();
        let data = storage.wandering_trader_data();
        let spawn_delay = data.spawn_delay() - DEFAULT_TICK_DELAY;
        data.set_spawn_delay(spawn_delay);
        if spawn_delay > 0 {
            return;
        }
        data.set_spawn_delay(DEFAULT_SPAWN_DELAY);
        let chance_to_spawn = data.spawn_chance();
        let new_spawn_chance = (chance_to_spawn + SPAWN_CHANCE_INCREASE).clamp(MIN_SPAWN_CHANCE, MAX_SPAWN_CHANCE);
        data.set_spawn_chance(new_spawn_chance);
        if self.random.next_int_bounded(100) <= chance_to_spawn {
            drop(storage);
            if self.spawn(level) {
                level.server_data_storage.borrow_mut().wandering_trader_data().set_spawn_chance(MIN_SPAWN_CHANCE);
            }
        }
    }

    fn spawn(&mut self, level: &mut ServerLevel) -> bool {
        let Some(player) = level.get_random_player() else { return true };
        if self.random.next_int_bounded(SPAWN_ONE_IN_X_CHANCE) != 0 {
            return false;
        }
        let player_pos = level.entity(player).block_position();
        let radius = 48;
        let poi_pos = level.chunks.poi_manager.find(&|poi_type| poi_type == PoiType::Meeting, |_| true, player_pos, radius, Occupancy::Any);
        let reference_pos = poi_pos.unwrap_or(player_pos);
        let Some(spawn_position) = self.find_spawn_position_near(level, reference_pos, radius) else { return false };
        if !self.has_enough_space(level, spawn_position) {
            return false;
        }
        if wafer_data::tag("minecraft:worldgen/biome", "minecraft:without_wandering_trader_spawns").contains(&(level.get_biome(spawn_position).0 as i32)) {
            return false;
        }
        let Some(trader) = level.spawn_entity_type("minecraft:wandering_trader", spawn_position, EntitySpawnReason::Event) else { return false };
        for _ in 0..2 {
            self.try_to_spawn_llama_for(level, trader, 4);
        }
        let mob = level.mob_mut(trader).expect("a wandering trader");
        WanderingTrader::set_despawn_delay(mob, 48000);
        WanderingTrader::set_wander_target(mob, Some(reference_pos));
        mob.set_home_to(reference_pos, 16);
        true
    }

    fn try_to_spawn_llama_for(&mut self, level: &mut ServerLevel, trader: EntityId, radius: i32) {
        let trader_pos = level.entity(trader).block_position();
        if let Some(spawn_position) = self.find_spawn_position_near(level, trader_pos, radius)
            && let Some(llama) = level.spawn_entity_type("minecraft:trader_llama", spawn_position, EntitySpawnReason::Event)
        {
            level.set_leashed_to(llama, trader, true);
        }
    }

    fn find_spawn_position_near(&mut self, level: &mut ServerLevel, reference_position: BlockPos, radius: i32) -> Option<BlockPos> {
        let heightmap = spawn_placements::get_heightmap_type(Some(&entity_types::WANDERING_TRADER));
        for _ in 0..NUMBER_OF_SPAWN_ATTEMPTS {
            let x_position = reference_position.x + self.random.next_int_bounded(radius * 2) - radius;
            let z_position = reference_position.z + self.random.next_int_bounded(radius * 2) - radius;
            let y_position = level.get_height(heightmap, x_position, z_position);
            let spawn_pos = BlockPos::new(x_position, y_position, z_position);
            if spawn_placements::is_spawn_position_ok(&entity_types::WANDERING_TRADER, level, spawn_pos) {
                return Some(spawn_pos);
            }
        }
        None
    }

    fn has_enough_space(&self, level: &mut ServerLevel, spawn_pos: BlockPos) -> bool {
        for pos in BlockPos::between_closed(spawn_pos, spawn_pos.offset(1, 2, 1)) {
            if !level.block_state(pos).get_collision_shape(pos).is_empty() {
                return false;
            }
        }
        true
    }
}
