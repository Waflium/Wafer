use super::target_goal::TargetGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

pub struct OwnerHurtTargetGoal {
    pub target_goal: TargetGoal,
    owner_last_hurt: Option<EntityId>,
    timestamp: i32,
}

impl OwnerHurtTargetGoal {
    pub fn new() -> OwnerHurtTargetGoal {
        OwnerHurtTargetGoal { target_goal: TargetGoal::new(false, false), owner_last_hurt: None, timestamp: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_tame(mob) || level.is_ordered_to_sit(mob) {
            return false;
        }
        let Some((_, owner)) = level.get_owner_in_any_dimension(mob) else { return false };
        let living = level.living(owner);
        self.owner_last_hurt = living.last_hurt_mob.filter(|&last_hurt_mob| level.entity_ref(last_hurt_mob).is_some());
        let ts = living.last_hurt_mob_timestamp;
        ts != self.timestamp
            && self.target_goal.can_attack(level, mob, self.owner_last_hurt, &TargetingConditions::for_combat())
            && level.wants_to_attack(mob, self.owner_last_hurt.expect("a victim"), owner)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, self.owner_last_hurt);
        if let Some((_, owner)) = level.get_owner_in_any_dimension(mob) {
            self.timestamp = level.living(owner).last_hurt_mob_timestamp;
        }
        self.target_goal.start();
    }
}
