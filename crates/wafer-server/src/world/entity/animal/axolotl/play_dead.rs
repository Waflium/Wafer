use wafer_data::mob_effects;

use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub struct PlayDead {
    behavior: BehaviorState,
}

impl Default for PlayDead {
    fn default() -> PlayDead {
        PlayDead {
            behavior: BehaviorState::with_time_out(
                vec![(memory_module_type::PLAY_DEAD_TICKS.erase(), MemoryStatus::ValuePresent), (memory_module_type::HURT_BY_ENTITY.erase(), MemoryStatus::ValuePresent)],
                200,
            ),
        }
    }
}

impl Behavior for PlayDead {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.entity(body).is_in_water()
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.entity(body).is_in_water() && level.get_brain(body).has_memory_value(memory_module_type::PLAY_DEAD_TICKS)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::LOOK_TARGET);
        level.add_effect(body, MobEffectInstance::new(mob_effects::REGENERATION, 200, 0), None);
    }
}
