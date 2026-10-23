use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::game_rule;

use crate::server::level::ServerLevel;
use crate::stats::stats;
use crate::stats::stats_counter::StatsCounter;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::level::natural_spawner;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::{BlockGetter, LevelReader};

#[derive(Default)]
pub struct PhantomSpawner {
    next_tick: i32,
}

impl PhantomSpawner {
    pub fn tick(&mut self, level: &mut ServerLevel, spawn_enemies: bool) {
        if !spawn_enemies || !level.game_rules.get_bool(game_rule::SPAWN_PHANTOMS) {
            return;
        }
        self.next_tick -= 1;
        if self.next_tick > 0 {
            return;
        }
        self.next_tick += (60 + level.random.next_int_bounded(60)) * 20;
        let has_sky_light = level.dimension_type().has_skylight;
        if level.sky_darken() < 5 && has_sky_light {
            return;
        }
        let players: Vec<_> = level.players.keys().copied().collect();
        for player in players {
            if level.is_spectator(player) {
                continue;
            }
            let player_pos = level.entity(player).block_position();
            if has_sky_light && (player_pos.y < level.get_sea_level() || !level.can_see_sky(player_pos)) {
                continue;
            }
            let difficulty = level.current_difficulty_at(player_pos);
            if !difficulty.is_harder_than(level.random.next_float() * 3.0) {
                continue;
            }
            let value = level.player(player).stats.get_value(stats::TIME_SINCE_REST).clamp(1, i32::MAX);
            if level.random.next_int_bounded(value) < 72000 {
                continue;
            }
            let random = &mut level.random;
            let up = 20 + random.next_int_bounded(15);
            let east = -10 + random.next_int_bounded(21);
            let south = -10 + random.next_int_bounded(21);
            let spawn_pos = player_pos.offset(0, up, 0).offset(east, 0, 0).offset(0, 0, south);
            let block_state = level.block_state(spawn_pos);
            let fluid_state = level.fluid_state(spawn_pos);
            if !natural_spawner::is_valid_empty_spawn_block(level, spawn_pos, block_state, fluid_state,&entity_types::PHANTOM) {
                continue;
            }
            let mut group_data = None;
            let group_size = 1 + level.random.next_int_bounded(difficulty.get_difficulty().id() + 1);
            for _ in 0..group_size {
                let Some(mut phantom) = ServerLevel::create_entity("minecraft:phantom") else { continue };
                let pos = Vec3::at_bottom_center_of(spawn_pos);
                phantom.entity_mut().snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
                let (phantom, data) = level.finalize_spawn_detached(phantom, &difficulty, EntitySpawnReason::Natural, group_data);
                group_data = data;
                level.add_fresh_entity_with_passengers(phantom);
            }
        }
    }
}
