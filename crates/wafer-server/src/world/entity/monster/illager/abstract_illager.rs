use wafer_data::entity_type_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::goal::open_door_goal::OpenDoorGoal;

pub struct RaiderOpenDoorGoal {
    pub open_door_goal: OpenDoorGoal,
}

impl RaiderOpenDoorGoal {
    pub fn new() -> RaiderOpenDoorGoal {
        RaiderOpenDoorGoal { open_door_goal: OpenDoorGoal::new(false) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.open_door_goal.door_interact_goal.can_use(level, mob) && level.raider_has_active_raid(mob)
    }
}

impl ServerLevel {
    pub fn is_abstract_illager(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(crate::world::entity::mob::Mob::is_abstract_illager)
    }

    pub fn abstract_illager_can_attack(&self, id: EntityId, target: EntityId) -> bool {
        let is_baby_villager = self.mob(target).is_some_and(|mob| mob.abstract_villager().is_some() && mob.is_baby());
        if is_baby_villager { false } else { self.mob_can_attack(id, target) }
    }

    pub fn abstract_illager_considers_entity_as_ally(&self, id: EntityId, other: EntityId) -> bool {
        if self.entity_considers_entity_as_ally(id, other) {
            return true;
        }
        if !entity_type_tags::ILLAGER_FRIENDS.contains(self.entity(other).entity_type as usize) {
            return false;
        }
        self.team_of(id).is_none() && self.team_of(other).is_none()
    }
}
