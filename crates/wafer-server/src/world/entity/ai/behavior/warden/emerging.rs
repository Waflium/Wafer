use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityId, sound_event};

pub struct Emerging {
    state: BehaviorState,
}

impl Emerging {
    pub fn new(ticks: i32) -> Emerging {
        let entry_condition = vec![
            (memory_module_type::IS_EMERGING.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
        ];
        Emerging { state: BehaviorState::with_time_out(entry_condition, ticks) }
    }
}

impl Behavior for Emerging {
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
        level.set_entity_pose(body, Pose::Emerging);
        level.entity_play_sound(body, sound_event("minecraft:entity.warden.emerge"), 5.0, 1.0);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.entity(body).pose() == Pose::Emerging {
            level.set_entity_pose(body, Pose::Standing);
        }
    }
}
