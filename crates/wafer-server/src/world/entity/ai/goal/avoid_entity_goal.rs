use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::entity::entity_selector;
use crate::world::level::pathfinder::path::Path;

fn predicate_on_avoid_entity(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    entity_selector::no_creative_or_spectator(level, target)
}

pub type PredicateOnAvoidEntity = fn(EntityId, EntityId, &ServerLevel) -> bool;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AvoidClass {
    EntityType(&'static str),
    Monster,
    Zombie,
    Guardian,
}

impl AvoidClass {
    fn is_instance(self, level: &ServerLevel, entity: EntityId) -> bool {
        match self {
            AvoidClass::EntityType(entity_type) => wafer_data::registry::ENTITY_TYPE[level.entity(entity).entity_type as usize] == entity_type,
            AvoidClass::Monster => level.is_monster(entity),
            AvoidClass::Zombie => level.mob(entity).is_some_and(|mob| mob.is_zombie()),
            AvoidClass::Guardian => level.is_guardian(entity),
        }
    }
}

pub struct AvoidEntityGoal {
    walk_speed_modifier: f64,
    sprint_speed_modifier: f64,
    pub to_avoid: Option<EntityId>,
    max_dist: f32,
    path: Option<Path>,
    avoid_class: AvoidClass,
    predicate_on_avoid_entity: PredicateOnAvoidEntity,
}

impl AvoidEntityGoal {
    pub fn new(avoid_class: &'static str, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64) -> AvoidEntityGoal {
        AvoidEntityGoal::of_class(AvoidClass::EntityType(avoid_class), max_dist, walk_speed_modifier, sprint_speed_modifier)
    }

    pub fn of_class(avoid_class: AvoidClass, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64) -> AvoidEntityGoal {
        AvoidEntityGoal { walk_speed_modifier, sprint_speed_modifier, to_avoid: None, max_dist, path: None, avoid_class, predicate_on_avoid_entity }
    }

    pub fn of_class_with_predicate(avoid_class: AvoidClass, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64, predicate_on_avoid_entity: PredicateOnAvoidEntity) -> AvoidEntityGoal {
        AvoidEntityGoal { predicate_on_avoid_entity, ..AvoidEntityGoal::of_class(avoid_class, max_dist, walk_speed_modifier, sprint_speed_modifier) }
    }

    pub fn with_predicate_on_avoid_entity(
        avoid_class: &'static str,
        max_dist: f32,
        walk_speed_modifier: f64,
        sprint_speed_modifier: f64,
        predicate_on_avoid_entity: PredicateOnAvoidEntity,
    ) -> AvoidEntityGoal {
        AvoidEntityGoal { predicate_on_avoid_entity, ..AvoidEntityGoal::new(avoid_class, max_dist, walk_speed_modifier, sprint_speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let max_dist = self.max_dist as f64;
        let area = level.entity(mob).bounding_box().inflate(max_dist, 3.0, max_dist);
        let avoid_class = self.avoid_class;
        let mut entities = level.get_entities(None, &area);
        entities.retain(|&entity| avoid_class.is_instance(level, entity));
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let predicate_on_avoid_entity = self.predicate_on_avoid_entity;
        let selector = move |target: EntityId, level: &ServerLevel| predicate_on_avoid_entity(mob, target, level);
        let avoid_entity_targeting = TargetingConditions::for_combat().range(self.max_dist as f64).selector(&selector);
        let to_avoid = level.get_nearest_entity(&entities, &avoid_entity_targeting, Some(mob), x, y, z);
        self.to_avoid = to_avoid;
        let Some(to_avoid) = self.to_avoid else { return false };
        let avoid_pos = level.entity(to_avoid).position();
        let Some(pos) = default_random_pos::get_pos_away(level, mob, 16, 7, avoid_pos) else { return false };
        if level.entity(to_avoid).distance_to_sqr_xyz(pos.x, pos.y, pos.z) < level.entity(to_avoid).distance_to_sqr(level.entity(mob)) {
            return false;
        }
        self.path = level.get_navigation(mob, |navigation, level| navigation.create_path_to(level, mob, pos.x, pos.y, pos.z, 0));
        self.path.is_some()
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a mob").ai.navigation.is_done()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (path, walk_speed_modifier) = (self.path.clone(), self.walk_speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to_path(level, mob, path, walk_speed_modifier));
    }

    pub fn stop(&mut self) {
        self.to_avoid = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(to_avoid) = self.to_avoid.filter(|&to_avoid| level.entity_ref(to_avoid).is_some()) else { return };
        let speed_modifier = if level.entity(mob).distance_to_sqr(level.entity(to_avoid)) < 49.0 { self.sprint_speed_modifier } else { self.walk_speed_modifier };
        level.mob_mut(mob).expect("a mob").ai.navigation.set_speed_modifier(speed_modifier);
    }
}
