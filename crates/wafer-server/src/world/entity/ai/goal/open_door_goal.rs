use super::door_interact_goal::DoorInteractGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct OpenDoorGoal {
    pub door_interact_goal: DoorInteractGoal,
    close_door: bool,
    forget_time: i32,
}

impl OpenDoorGoal {
    pub fn new(close_door_after: bool) -> OpenDoorGoal {
        OpenDoorGoal { door_interact_goal: DoorInteractGoal::default(), close_door: close_door_after, forget_time: 0 }
    }

    pub fn can_continue_to_use(&self) -> bool {
        self.close_door && self.forget_time > 0 && self.door_interact_goal.can_continue_to_use()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.forget_time = 20;
        self.door_interact_goal.set_open(level, mob, true);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.door_interact_goal.set_open(level, mob, false);
    }

    pub fn tick(&mut self, level: &ServerLevel, mob: EntityId) {
        self.forget_time -= 1;
        self.door_interact_goal.tick(level, mob);
    }
}
