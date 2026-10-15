use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;

const CROAK_TICKS: i32 = 60;
const TIME_OUT_DURATION: i32 = 100;

pub struct Croak {
    behavior: BehaviorState,
    croak_counter: i32,
}

impl Default for Croak {
    fn default() -> Croak {
        Croak { behavior: BehaviorState::with_time_out(vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)], TIME_OUT_DURATION), croak_counter: 0 }
    }
}

impl Behavior for Croak {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.entity(body).pose() == Pose::Standing
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        self.croak_counter < CROAK_TICKS
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if !level.entity(body).is_in_liquid() {
            level.set_entity_pose(body, Pose::Croaking);
            self.croak_counter = 0;
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.set_entity_pose(body, Pose::Standing);
    }

    fn tick(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {
        self.croak_counter += 1;
    }
}
