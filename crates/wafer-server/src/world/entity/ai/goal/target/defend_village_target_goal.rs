use super::target_goal::TargetGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct DefendVillageTargetGoal {
    pub target_goal: TargetGoal,
    potential_target: Option<EntityId>,
}

impl DefendVillageTargetGoal {
    pub fn new() -> DefendVillageTargetGoal {
        DefendVillageTargetGoal { target_goal: TargetGoal::new(false, true), potential_target: None }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, _golem: EntityId) -> bool {
        let Some(potential_target) = self.potential_target else { return false };
        !(level.is_player(potential_target) && (level.is_spectator(potential_target) || level.player(potential_target).game_mode.game_type.is_creative()))
    }

    pub fn start(&mut self, level: &mut ServerLevel, golem: EntityId) {
        level.set_target(golem, self.potential_target);
        self.target_goal.start();
    }
}
