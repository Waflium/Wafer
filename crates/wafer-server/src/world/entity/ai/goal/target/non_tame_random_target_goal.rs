use super::nearest_attackable_target_goal::{NearestAttackableTargetGoal, Selector, TargetType};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

pub struct NonTameRandomTargetGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
    target_conditions_range: f64,
}

impl NonTameRandomTargetGoal {
    pub fn new(target_type: TargetType, must_see: bool, selector: Option<Selector>, follow_distance: f64) -> NonTameRandomTargetGoal {
        NonTameRandomTargetGoal { nearest_attackable_target_goal: NearestAttackableTargetGoal::with(target_type, 10, must_see, false, selector), target_conditions_range: follow_distance }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_tame(mob) && self.nearest_attackable_target_goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = self.nearest_attackable_target_goal.target.filter(|&target| level.entity_ref(target).is_some()) else { return false };
        let selector = self.nearest_attackable_target_goal.selector.map(|selector| move |target: EntityId, level: &ServerLevel| selector(mob, target, level));
        let mut target_conditions = TargetingConditions::for_combat().range(self.target_conditions_range);
        if let Some(selector) = &selector {
            target_conditions = target_conditions.selector(selector);
        }
        target_conditions.test(level, Some(mob), target)
    }
}
