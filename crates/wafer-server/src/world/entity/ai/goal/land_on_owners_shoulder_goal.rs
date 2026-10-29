use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct LandOnOwnersShoulderGoal {
    is_sitting_on_shoulder: bool,
}

impl LandOnOwnersShoulderGoal {
    pub fn can_use(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let Some((_, owner)) = level.get_owner_in_any_dimension(entity).filter(|&(_, owner)| level.is_player(owner)) else { return false };
        let p = level.player(owner);
        let owner_that_can_be_sat_on = !level.is_spectator(owner) && !p.abilities_flying() && !p.entity.is_in_water() && !p.entity.is_in_powder_snow;
        !level.is_ordered_to_sit(entity) && owner_that_can_be_sat_on && level.can_sit_on_shoulder(entity)
    }

    pub fn is_interruptable(&self) -> bool {
        !self.is_sitting_on_shoulder
    }

    pub fn start(&mut self) {
        self.is_sitting_on_shoulder = false;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, entity: EntityId) {
        if self.is_sitting_on_shoulder || level.is_in_sitting_pose(entity) || level.is_leashed(entity) {
            return;
        }
        if let Some((_, owner)) = level.get_owner_in_any_dimension(entity).filter(|&(_, owner)| level.is_player(owner))
            && level.entity(entity).bounding_box().intersects(&level.entity(owner).bounding_box())
        {
            self.is_sitting_on_shoulder = level.set_entity_on_shoulder(entity, owner);
        }
    }
}
