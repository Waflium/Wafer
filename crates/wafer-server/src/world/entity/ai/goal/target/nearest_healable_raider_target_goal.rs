use wafer_core::random::RandomSource;

use super::nearest_attackable_target_goal::{NearestAttackableTargetGoal, Selector, TargetType};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::goal::goal::reduced_tick_delay;

const DEFAULT_COOLDOWN: i32 = 200;

pub struct NearestHealableRaiderTargetGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
    cooldown: i32,
}

impl NearestHealableRaiderTargetGoal {
    pub fn new(target_type: TargetType, must_see: bool, subselector: Option<Selector>) -> NearestHealableRaiderTargetGoal {
        NearestHealableRaiderTargetGoal { nearest_attackable_target_goal: NearestAttackableTargetGoal::with(target_type, 500, must_see, false, subselector), cooldown: 0 }
    }

    pub fn get_cooldown(&self) -> i32 {
        self.cooldown
    }

    pub fn decrement_cooldown(&mut self) {
        self.cooldown -= 1;
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.cooldown > 0 || !level.entity_mut(mob).random.next_boolean() {
            return false;
        }
        if !level.raider_has_active_raid(mob) {
            return false;
        }
        self.nearest_attackable_target_goal.find_target(level, mob);
        self.nearest_attackable_target_goal.target.is_some()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.cooldown = reduced_tick_delay(DEFAULT_COOLDOWN);
        self.nearest_attackable_target_goal.start(level, mob);
    }
}
