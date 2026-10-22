use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityId, RemovalReason, sound_event};

pub struct Digging {
    state: BehaviorState,
}

impl Digging {
    pub fn new(ticks: i32) -> Digging {
        let entry_condition = vec![(memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValueAbsent), (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)];
        Digging { state: BehaviorState::with_time_out(entry_condition, ticks) }
    }
}

impl Behavior for Digging {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.entity(body).removal.is_none()
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let e = level.entity(body);
        e.on_ground || e.is_in_liquid()
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if level.entity(body).on_ground {
            level.set_entity_pose(body, Pose::Digging);
            level.entity_play_sound(body, sound_event("minecraft:entity.warden.dig"), 5.0, 1.0);
        } else {
            level.entity_play_sound(body, sound_event("minecraft:entity.warden.agitated"), 5.0, 1.0);
            self.stop(level, body, timestamp);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.entity(body).removal.is_none() {
            level.remove(body, RemovalReason::Discarded);
        }
    }
}
