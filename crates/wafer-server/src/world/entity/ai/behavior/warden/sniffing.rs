use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;
use crate::world::entity::monster::warden::warden_ai;
use crate::world::entity::{EntityId, sound_event};

const ANGER_FROM_SNIFFING_MAX_DISTANCE_XZ: f64 = 6.0;
const ANGER_FROM_SNIFFING_MAX_DISTANCE_Y: f64 = 20.0;

pub struct Sniffing {
    state: BehaviorState,
}

impl Sniffing {
    pub fn new(ticks: i32) -> Sniffing {
        let entry_condition = vec![
            (memory_module_type::IS_SNIFFING.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
            (memory_module_type::NEAREST_ATTACKABLE.erase(), MemoryStatus::Registered),
            (memory_module_type::DISTURBANCE_LOCATION.erase(), MemoryStatus::Registered),
            (memory_module_type::SNIFF_COOLDOWN.erase(), MemoryStatus::Registered),
        ];
        Sniffing { state: BehaviorState::with_time_out(entry_condition, ticks) }
    }
}

impl Behavior for Sniffing {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        true
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.entity_play_sound(body, sound_event("minecraft:entity.warden.sniff"), 5.0, 1.0);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.entity(body).pose() == Pose::Sniffing {
            level.set_entity_pose(body, Pose::Standing);
        }
        level.get_brain_mut(body).erase_memory(memory_module_type::IS_SNIFFING);
        let Some(entity) = level.get_brain(body).get_memory(memory_module_type::NEAREST_ATTACKABLE).copied().filter(|&entity| level.warden_can_target_entity(body, Some(entity))) else { return };
        let target_position = level.entity(entity).position();
        if level.entity(body).position().closer_than_xz_y(target_position, ANGER_FROM_SNIFFING_MAX_DISTANCE_XZ, ANGER_FROM_SNIFFING_MAX_DISTANCE_Y) {
            level.warden_increase_anger_at_default(body, Some(entity));
        }
        if !level.get_brain(body).has_memory_value(memory_module_type::DISTURBANCE_LOCATION) {
            let position = level.entity(entity).block_position();
            warden_ai::set_disturbance_location(level, body, position);
        }
    }
}
