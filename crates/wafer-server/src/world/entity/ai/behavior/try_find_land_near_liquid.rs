use wafer_core::{BlockPos, Direction};
use wafer_data::Tag;

use super::block_pos_tracker::BlockPosTracker;
use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::level::BlockGetter;
use crate::world::phys::shapes::collision_context::CollisionContext;

pub fn create(range: i32, speed_modifier: f32, fluid_tag: &'static Tag) -> OneShot {
    let mut next_ok_start_time = 0i64;
    behavior_builder::create(|i| {
        i.absent(memory_module_type::ATTACK_TARGET);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            let body_block_pos = level.entity(body).block_position();
            if level.fluid_state(body_block_pos).is(fluid_tag) {
                return false;
            }
            if timestamp < next_ok_start_time {
                next_ok_start_time = timestamp + 40;
                return true;
            }
            let context = CollisionContext::of(level, body);
            let found = BlockPos::within_box_by_manhattan_distance(body_block_pos, range, range, range).filter(|pos| pos.differs_horizontally(body_block_pos)).find(|&pos| {
                let state = level.block_state(pos);
                if !context.get_collision_shape(state, pos).0.is_empty() {
                    return false;
                }
                let below = pos.relative(Direction::Down);
                if context.get_collision_shape(level.block_state(below), pos).0.is_empty() {
                    return false;
                }
                Direction::PLANE_HORIZONTAL.into_iter().any(|direction| {
                    let test_pos = pos.relative(direction);
                    level.block_state(test_pos).is_air() && level.block_state(test_pos.relative(Direction::Down)).fluid_state().is(fluid_tag)
                })
            });
            if let Some(target_pos) = found {
                look_target.set(level, body, PositionTracker::BlockPos(BlockPosTracker::new(target_pos)));
                walk_target.set(level, body, WalkTarget::new(PositionTracker::BlockPos(BlockPosTracker::new(target_pos)), speed_modifier, 0));
            }
            next_ok_start_time = timestamp + 40;
            true
        }
    })
}
