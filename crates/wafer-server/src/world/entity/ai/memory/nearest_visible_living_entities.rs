use std::cell::Cell;
use std::rc::Rc;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::sensing::sensor;

#[derive(Debug)]
struct Entities {
    body: Option<EntityId>,
    nearby_entities: Vec<EntityId>,
    line_of_sight: Vec<Cell<Option<bool>>>,
}

#[derive(Clone, Debug)]
pub struct NearestVisibleLivingEntities(Rc<Entities>);

impl NearestVisibleLivingEntities {
    pub fn new(body: EntityId, living_entities: Vec<EntityId>) -> NearestVisibleLivingEntities {
        let line_of_sight = living_entities.iter().map(|_| Cell::new(None)).collect();
        NearestVisibleLivingEntities(Rc::new(Entities { body: Some(body), nearby_entities: living_entities, line_of_sight }))
    }

    pub fn empty() -> NearestVisibleLivingEntities {
        NearestVisibleLivingEntities(Rc::new(Entities { body: None, nearby_entities: Vec::new(), line_of_sight: Vec::new() }))
    }

    pub fn nearby_entities(&self) -> &[EntityId] {
        &self.0.nearby_entities
    }

    fn line_of_sight_test(&self, level: &mut ServerLevel, index: usize) -> bool {
        let Some(body) = self.0.body else { return false };
        if let Some(cached) = self.0.line_of_sight[index].get() {
            return cached;
        }
        let result = sensor::is_entity_targetable(level, body, self.0.nearby_entities[index]);
        self.0.line_of_sight[index].set(Some(result));
        result
    }

    pub fn find_closest(&self, level: &mut ServerLevel, mut filter: impl FnMut(&mut ServerLevel, EntityId) -> bool) -> Option<EntityId> {
        (0..self.0.nearby_entities.len()).find(|&i| self.test(level, i, &mut filter)).map(|i| self.0.nearby_entities[i])
    }

    pub fn find_all(&self, level: &mut ServerLevel, mut filter: impl FnMut(&mut ServerLevel, EntityId) -> bool) -> Vec<EntityId> {
        (0..self.0.nearby_entities.len()).filter(|&i| self.test(level, i, &mut filter)).map(|i| self.0.nearby_entities[i]).collect()
    }

    pub fn contains(&self, level: &mut ServerLevel, target_entity: EntityId) -> bool {
        match self.0.nearby_entities.iter().position(|&e| e == target_entity) {
            Some(index) => self.0.line_of_sight[index].get().unwrap_or_else(|| level.entity_ref(target_entity).is_some() && self.line_of_sight_test(level, index)),
            None => false,
        }
    }

    pub fn contains_matching(&self, level: &mut ServerLevel, mut filter: impl FnMut(&mut ServerLevel, EntityId) -> bool) -> bool {
        (0..self.0.nearby_entities.len()).any(|i| self.test(level, i, &mut filter))
    }

    fn test(&self, level: &mut ServerLevel, index: usize, filter: &mut impl FnMut(&mut ServerLevel, EntityId) -> bool) -> bool {
        let entity = self.0.nearby_entities[index];
        level.entity_ref(entity).is_some() && filter(level, entity) && self.line_of_sight_test(level, index)
    }
}
