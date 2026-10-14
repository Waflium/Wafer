use super::nearest_attackable_target_goal::{NearestAttackableTargetGoal, Selector, TargetType};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct NearestAttackableWitchTargetGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
    can_attack: bool,
}

impl NearestAttackableWitchTargetGoal {
    pub fn new(target_type: TargetType, random_interval: i32, must_see: bool, must_reach: bool, subselector: Option<Selector>) -> NearestAttackableWitchTargetGoal {
        NearestAttackableWitchTargetGoal { nearest_attackable_target_goal: NearestAttackableTargetGoal::with(target_type, random_interval, must_see, must_reach, subselector), can_attack: true }
    }

    pub fn set_can_attack(&mut self, can_attack: bool) {
        self.can_attack = can_attack;
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_attack && self.nearest_attackable_target_goal.can_use(level, mob)
    }
}
