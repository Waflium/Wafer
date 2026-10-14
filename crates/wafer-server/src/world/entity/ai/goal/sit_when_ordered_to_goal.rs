use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::tamable_animal::TELEPORT_WHEN_DISTANCE_IS_SQ;

pub struct SitWhenOrderedToGoal;

impl SitWhenOrderedToGoal {
    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.is_ordered_to_sit(mob)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let ordered_to_sit = level.is_ordered_to_sit(mob);
        if !ordered_to_sit && !level.is_tame(mob) {
            return false;
        }
        let e = level.entity(mob);
        if e.is_in_water() {
            return false;
        }
        if !e.on_ground {
            return false;
        }
        let Some(owner) = level.get_owner(mob) else { return true };
        if level.entity(mob).distance_to_sqr(level.entity(owner)) < TELEPORT_WHEN_DISTANCE_IS_SQ && level.get_last_hurt_by_mob(owner).is_some() { false } else { ordered_to_sit }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.get_navigation(mob, |navigation, _| navigation.stop());
        level.set_in_sitting_pose(mob, true);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_in_sitting_pose(mob, false);
    }
}
