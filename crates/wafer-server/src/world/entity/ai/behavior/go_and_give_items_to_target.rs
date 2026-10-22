use wafer_core::Vec3;

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

const CLOSE_ENOUGH_DISTANCE_TO_TARGET: i32 = 3;

pub struct GoAndGiveItemsToTarget {
    behavior: BehaviorState,
    target_position_getter: fn(&mut ServerLevel, EntityId) -> Option<PositionTracker>,
    speed_modifier: f32,
    item_thrower: fn(&mut ServerLevel, EntityId, Vec3),
    cooldown_memory: MemoryModuleType<i32>,
    cooldown_duration: i32,
    has_item_predicate: fn(&ServerLevel, EntityId) -> bool,
}

impl GoAndGiveItemsToTarget {
    pub fn new(
        target_position_getter: fn(&mut ServerLevel, EntityId) -> Option<PositionTracker>,
        speed_modifier: f32,
        timeout_duration: i32,
        item_thrower: fn(&mut ServerLevel, EntityId, Vec3),
        cooldown_memory: MemoryModuleType<i32>,
        cooldown_duration: i32,
        has_item_predicate: fn(&ServerLevel, EntityId) -> bool,
    ) -> GoAndGiveItemsToTarget {
        GoAndGiveItemsToTarget {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::Registered),
                    (cooldown_memory.erase(), MemoryStatus::Registered),
                ],
                timeout_duration,
            ),
            target_position_getter,
            speed_modifier,
            item_thrower,
            cooldown_memory,
            cooldown_duration,
            has_item_predicate,
        }
    }

    fn can_throw_item_to_target(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        (self.has_item_predicate)(level, body) && (self.target_position_getter)(level, body).is_some()
    }
}

impl Behavior for GoAndGiveItemsToTarget {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.can_throw_item_to_target(level, body)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        self.can_throw_item_to_target(level, body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if let Some(position_tracker) = (self.target_position_getter)(level, body) {
            behavior_utils::set_walk_and_look_target_memories(level, body, position_tracker, self.speed_modifier, CLOSE_ENOUGH_DISTANCE_TO_TARGET);
        }
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let Some(deposit_target) = (self.target_position_getter)(level, body) else { return };
        let deposit_position = deposit_target.current_position(level);
        let distance_to_target = deposit_position.distance_to(level.entity(body).eye_position());
        if distance_to_target < 3.0 {
            let target_pos = deposit_target.current_position(level);
            (self.item_thrower)(level, body, target_pos);
            level.get_brain_mut(body).set_memory(self.cooldown_memory, self.cooldown_duration);
        }
    }
}
