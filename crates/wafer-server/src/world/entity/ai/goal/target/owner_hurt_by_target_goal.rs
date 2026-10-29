use wafer_data::damage_type_tags;

use super::target_goal::TargetGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

pub struct OwnerHurtByTargetGoal {
    pub target_goal: TargetGoal,
    owner_last_hurt_by: Option<EntityId>,
    timestamp: i32,
}

impl OwnerHurtByTargetGoal {
    pub fn new() -> OwnerHurtByTargetGoal {
        OwnerHurtByTargetGoal { target_goal: TargetGoal::new(false, false), owner_last_hurt_by: None, timestamp: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_tame(mob) || level.is_ordered_to_sit(mob) {
            return false;
        }
        let Some((_, owner)) = level.get_owner_in_any_dimension(mob) else { return false };
        let retaliates = level.get_last_damage_source_with_timeout(owner, 100).is_some_and(|last_damage_source| !last_damage_source.is(&damage_type_tags::NO_WOLF_RETALIATION));
        if !retaliates {
            return false;
        }
        self.owner_last_hurt_by = level.get_last_hurt_by_mob(owner);
        let ts = level.living(owner).last_hurt_by_mob_timestamp;
        ts != self.timestamp
            && self.target_goal.can_attack(level, mob, self.owner_last_hurt_by, &TargetingConditions::for_combat())
            && level.wants_to_attack(mob, self.owner_last_hurt_by.expect("an attacker"), owner)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, self.owner_last_hurt_by);
        if let Some((_, owner)) = level.get_owner_in_any_dimension(mob) {
            self.timestamp = level.living(owner).last_hurt_by_mob_timestamp;
        }
        self.target_goal.start();
    }
}
