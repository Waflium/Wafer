use wafer_data::mob_effects;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub struct ShootWhenStuck {
    state: BehaviorState,
}

impl Default for ShootWhenStuck {
    fn default() -> ShootWhenStuck {
        let entry_condition = vec![
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::BREEZE_JUMP_INHALING.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_JUMP_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_SHOOT.erase(), MemoryStatus::ValueAbsent),
        ];
        ShootWhenStuck { state: BehaviorState::new(entry_condition) }
    }
}

impl Behavior for ShootWhenStuck {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, breeze: EntityId) -> bool {
        level.is_passenger(breeze) || level.entity(breeze).is_in_water() || level.has_effect(breeze, mob_effects::LEVITATION)
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        false
    }

    fn start(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        level.get_brain_mut(breeze).set_memory_with_expiry(memory_module_type::BREEZE_SHOOT, (), 60);
    }
}
