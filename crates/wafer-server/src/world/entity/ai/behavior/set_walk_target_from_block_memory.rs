use wafer_core::{BlockPos, Vec3};

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::default_random_pos;

const MAX_TRIES: i32 = 1000;

pub fn create(memory_type: MemoryModuleType<GlobalPos>, speed_modifier: f32, close_enough_dist: i32, too_far_distance: i32, too_long_unreachable_duration: i32) -> OneShot {
    behavior_builder::create(|i| {
        let cant_reach_since = i.registered(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let memory = i.present(memory_type);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            let target_pos = memory.get(level, body);
            let cant_reach_target_since = cant_reach_since.try_get(level, body);
            if target_pos.dimension == level.dimension && cant_reach_target_since.is_none_or(|since| level.game_time - since <= too_long_unreachable_duration as i64) {
                let body_pos = level.entity(body).block_position();
                if target_pos.pos.dist_manhattan(body_pos) > too_far_distance {
                    let mut towards_target_pos: Option<Vec3> = None;
                    let mut tries = 0;
                    while towards_target_pos.is_none_or(|pos| BlockPos::containing_vec(pos).dist_manhattan(level.entity(body).block_position()) > too_far_distance) {
                        towards_target_pos = default_random_pos::get_pos_towards(level, body, 15, 7, Vec3::at_bottom_center_of(target_pos.pos), std::f32::consts::FRAC_PI_2 as f64);
                        tries += 1;
                        if tries == MAX_TRIES {
                            level.release_poi(body, memory_type);
                            memory.erase(level, body);
                            cant_reach_since.set(level, body, timestamp);
                            return true;
                        }
                    }
                    walk_target.set(level, body, WalkTarget::of_vec(towards_target_pos.expect("a position towards the target"), speed_modifier, close_enough_dist));
                } else if target_pos.pos.dist_manhattan(body_pos) > close_enough_dist {
                    walk_target.set(level, body, WalkTarget::of_block_pos(target_pos.pos, speed_modifier, close_enough_dist));
                }
            } else {
                level.release_poi(body, memory_type);
                memory.erase(level, body);
                cant_reach_since.set(level, body, timestamp);
            }
            true
        }
    })
}
