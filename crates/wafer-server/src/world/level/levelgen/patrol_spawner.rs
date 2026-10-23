use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::game_rule;

use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::entity::LevelEntity;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::monster::patrolling_monster;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::BlockGetter;
use crate::world::level::natural_spawner;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

#[derive(Default)]
pub struct PatrolSpawner {
    next_tick: i32,
}

impl PatrolSpawner {
    pub fn tick(&mut self, level: &mut ServerLevel, spawn_enemies: bool) {
        if !spawn_enemies || !level.game_rules.get_bool(game_rule::SPAWN_PATROLS) {
            return;
        }
        self.next_tick -= 1;
        if self.next_tick > 0 {
            return;
        }
        self.next_tick += 12000 + level.random.next_int_bounded(1200);
        if !level.is_bright_outside() || level.random.next_int_bounded(5) != 0 {
            return;
        }
        let player_count = level.players.len() as i32;
        if player_count < 1 {
            return;
        }
        let player = *level.players.get_index(level.random.next_int_bounded(player_count) as usize).expect("a player").0;
        if level.is_spectator(player) {
            return;
        }
        let player_pos = level.entity(player).block_position();
        if level.is_close_to_village(player_pos, 2) {
            return;
        }
        let random = &mut level.random;
        let x = (24 + random.next_int_bounded(24)) * if random.next_boolean() { -1 } else { 1 };
        let z = (24 + random.next_int_bounded(24)) * if random.next_boolean() { -1 } else { 1 };
        let mut spawn_pos = player_pos.offset(x, 0, z);
        let delta = 10;
        if !level.has_chunks_at(spawn_pos.x - delta, spawn_pos.z - delta, spawn_pos.x + delta, spawn_pos.z + delta) {
            return;
        }
        if !level.environment_attributes().get_value_at(environment_attributes::CAN_PILLAGER_PATROL_SPAWN, spawn_pos) {
            return;
        }
        let group_size = level.current_difficulty_at(spawn_pos).get_effective_difficulty().ceil() as i32 + 1;
        for i in 0..group_size {
            spawn_pos.y = level.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, spawn_pos).y;
            if i == 0 {
                if !self.spawn_patrol_member(level, spawn_pos, true) {
                    break;
                }
            } else {
                self.spawn_patrol_member(level, spawn_pos, false);
            }
            let random = &mut level.random;
            spawn_pos.x = spawn_pos.x + random.next_int_bounded(5) - random.next_int_bounded(5);
            spawn_pos.z = spawn_pos.z + random.next_int_bounded(5) - random.next_int_bounded(5);
        }
    }

    fn spawn_patrol_member(&mut self, level: &mut ServerLevel, pos: BlockPos, is_leader: bool) -> bool {
        let state = level.block_state(pos);
        if !natural_spawner::is_valid_empty_spawn_block(level, pos, state, state.fluid_state(),&entity_types::PILLAGER) {
            return false;
        }
        if !patrolling_monster::check_patrolling_monster_spawn_rules(&entity_types::PILLAGER, level, EntitySpawnReason::Patrol, pos) {
            return false;
        }
        let Some(mob) = ServerLevel::create_entity("minecraft:pillager") else { return false };
        let id = mob.entity().id;
        level.entities.insert(id, mob);
        if is_leader {
            level.mob_mut(id).and_then(|mob| mob.patrolling_monster_mut()).expect("a patrolling monster").set_patrol_leader(true);
            level.find_patrol_target(id);
        }
        level.entity_mut(id).set_pos(Vec3::new(pos.x as f64, pos.y as f64, pos.z as f64));
        let difficulty = level.current_difficulty_at(pos);
        level.finalize_spawn(id, &difficulty, EntitySpawnReason::Patrol, None);
        let mob: LevelEntity = level.entities.shift_remove(&id).expect("the patrol member");
        level.add_fresh_entity_with_passengers(mob);
        true
    }
}
