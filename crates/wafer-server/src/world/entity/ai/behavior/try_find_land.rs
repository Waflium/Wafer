use wafer_core::{BlockPos, Direction};
use wafer_data::{BlockState, fluid_tags};

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

const COOLDOWN_TICKS: i64 = 60;

pub fn create(range: i32, speed_modifier: f32) -> OneShot {
    let mut next_ok_start_time = 0i64;
    behavior_builder::create(|i| {
        i.absent(memory_module_type::ATTACK_TARGET);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            let body_block_pos = level.entity(body).block_position();
            if !level.fluid_state(body_block_pos).is(&fluid_tags::WATER) {
                return false;
            }
            if timestamp < next_ok_start_time {
                next_ok_start_time = timestamp + COOLDOWN_TICKS;
                return true;
            }
            let context = CollisionContext::of(level, body);
            let found = BlockPos::within_box_by_manhattan_distance(body_block_pos, range, range, range)
                .filter(|pos| pos.differs_horizontally(body_block_pos))
                .find(|&pos| {
                    let state = level.block_state(pos);
                    state.fluid_state().is_empty() && can_stand_on(level, pos, state, &context)
                });
            if let Some(target_pos) = found {
                look_target.set(level, body, PositionTracker::BlockPos(BlockPosTracker::new(target_pos)));
                walk_target.set(level, body, WalkTarget::new(PositionTracker::BlockPos(BlockPosTracker::new(target_pos)), speed_modifier, 1));
            }
            next_ok_start_time = timestamp + COOLDOWN_TICKS;
            true
        }
    })
}

fn can_stand_on(level: &mut ServerLevel, pos: BlockPos, state: BlockState, context: &CollisionContext) -> bool {
    if !level.get_state_collision_shape_in_context(state, pos, context).is_empty() {
        return false;
    }
    let below_pos = pos.relative(Direction::Down);
    let below_state = level.block_state(below_pos);
    level.is_face_sturdy_full_at(below_pos, below_state, Direction::Up)
}
