use wafer_core::BlockPos;
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
                next_ok_start_time = timestamp + 20 + 2;
                return true;
            }
            let body_position = level.entity(body).position();
            let mut found_pos: Option<BlockPos> = None;
            for pos in BlockPos::within_box_by_manhattan_distance(body_block_pos, range, range, range).filter(|pos| pos.differs_horizontally(body_block_pos)) {
                let state = level.block_state(pos);
                if !(state.block().is_instance_of("LiquidBlock") && state.fluid_state().is(fluid_tag)) {
                    continue;
                }
                let above_state = level.block_state(pos.above());
                if above_state.is_air() {
                    found_pos = Some(pos);
                    break;
                }
                if found_pos.is_none() && !pos.closer_to_center_than(body_position, 1.5) {
                    found_pos = Some(pos);
                }
            }
            if let Some(block_pos) = found_pos {
                look_target.set(level, body, PositionTracker::BlockPos(BlockPosTracker::new(block_pos)));
                walk_target.set(level, body, WalkTarget::new(PositionTracker::BlockPos(BlockPosTracker::new(block_pos)), speed_modifier, 0));
            }
            next_ok_start_time = timestamp + 40;
            true
        }
    })
}
