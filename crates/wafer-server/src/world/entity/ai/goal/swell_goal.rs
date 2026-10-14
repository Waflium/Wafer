use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::monster::creeper;

#[derive(Default)]
pub struct SwellGoal {
    target: Option<EntityId>,
}

impl SwellGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let target = level.get_target(mob);
        creeper::get_swell_dir(level.entity(mob)) > 0
            || target.is_some_and(|target| !level.is_dead_or_dying(target) && level.entity(mob).distance_to_sqr(level.entity(target)) < 9.0)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a creeper").ai.navigation.stop();
        self.target = level.get_target(mob);
    }

    pub fn stop(&mut self) {
        self.target = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let swell_dir = match self.target.filter(|&target| level.entity_ref(target).is_some() && !level.is_dead_or_dying(target)) {
            Some(target) if level.entity(mob).distance_to_sqr(level.entity(target)) > 49.0 => -1,
            Some(target) if !level.sensing_has_line_of_sight(mob, target) => -1,
            Some(_) => 1,
            None => -1,
        };
        creeper::set_swell_dir(level.entity_mut(mob), swell_dir);
    }
}
