use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};

pub struct ForceUnmount {
    state: BehaviorState,
}

impl Default for ForceUnmount {
    fn default() -> ForceUnmount {
        ForceUnmount { state: BehaviorState::new(Vec::new()) }
    }
}

impl Behavior for ForceUnmount {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.is_passenger(body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.un_ride(body);
    }
}
