use wafer_core::AABB;

use super::ServerLevel;
use super::game_mode::GameType;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::mob::MobKind;
use crate::world::entity::{EntityId, LevelEntity};

impl ServerLevel {
    pub fn get_nearest_player(&mut self, target_conditions: &TargetingConditions, source: Option<EntityId>, x: f64, y: f64, z: f64) -> Option<EntityId> {
        let mut best = -1.0;
        let mut result = None;
        for i in 0..self.players.len() {
            let player = *self.players.get_index(i).expect("a player").0;
            if target_conditions.test(self, source, player) {
                let dist = self.entity(player).distance_to_sqr_xyz(x, y, z);
                if best == -1.0 || dist < best {
                    best = dist;
                    result = Some(player);
                }
            }
        }
        result
    }

    pub fn get_nearby_players(&mut self, target_conditions: &TargetingConditions, source: EntityId, bb: &AABB) -> Vec<EntityId> {
        let mut found_players = Vec::new();
        for i in 0..self.players.len() {
            let player = *self.players.get_index(i).expect("a player").0;
            let e = self.entity(player);
            if bb.contains(e.x(), e.y(), e.z()) && target_conditions.test(self, Some(source), player) {
                found_players.push(player);
            }
        }
        found_players
    }

    pub fn get_nearest_player_to(&self, x: f64, y: f64, z: f64, range: f64, filter_out_creative: bool) -> Option<EntityId> {
        let mut best = -1.0;
        let mut result = None;
        for player in self.players.values() {
            let game_type = player.game_mode.game_type;
            if game_type == GameType::Spectator || filter_out_creative && game_type == GameType::Creative {
                continue;
            }
            let dist = player.entity.distance_to_sqr_xyz(x, y, z);
            if (range < 0.0 || dist < range * range) && (best == -1.0 || dist < best) {
                best = dist;
                result = Some(player.entity.id);
            }
        }
        result
    }

    pub fn has_nearby_alive_player(&self, x: f64, y: f64, z: f64, range: f64) -> bool {
        self.players.values().any(|player| {
            player.game_mode.game_type != GameType::Spectator && self.is_alive(player.entity.id) && {
                let player_dist = player.entity.distance_to_sqr_xyz(x, y, z);
                range < 0.0 || player_dist < range * range
            }
        })
    }

    pub fn is_unobstructed_by_entities(&self, source: Option<EntityId>, aabb: &AABB) -> bool {
        self.get_entities(source, aabb).into_iter().all(|entity| {
            self.entity(entity).is_removed()
                || !self.blocks_building(entity)
                || source.is_some_and(|source| self.is_passenger_of_same_vehicle(entity, source))
                || !self.entity(entity).bounding_box().intersects(aabb)
        })
    }

    pub fn get_nearest_entity(&mut self, entities: &[EntityId], target_conditions: &TargetingConditions, source: Option<EntityId>, x: f64, y: f64, z: f64) -> Option<EntityId> {
        let mut best = -1.0;
        let mut result = None;
        for &entity in entities {
            if target_conditions.test(self, source, entity) {
                let dist = self.entity(entity).distance_to_sqr_xyz(x, y, z);
                if best == -1.0 || dist < best {
                    best = dist;
                    result = Some(entity);
                }
            }
        }
        result
    }

    pub fn get_living_entities(&self, area: &AABB, predicate: impl Fn(&ServerLevel, EntityId) -> bool) -> Vec<EntityId> {
        let mut found = Vec::new();
        self.entity_manager.section_storage.for_each_accessible_non_empty_section(area, |section| {
            for &id in section.entities() {
                if self.is_living(id)
                    && let Some(entity) = self.entity_ref(id)
                    && !entity.is_removed()
                    && entity.bounding_box().intersects(area)
                    && predicate(self, id)
                {
                    found.push(id);
                }
            }
        });
        found
    }

    pub fn get_entities_of_class_same_kind(&self, mob: EntityId, bb: &AABB) -> Vec<EntityId> {
        let mut entities = self.get_entities(None, bb);
        entities.retain(|&entity| self.is_same_mob_class(mob, entity));
        entities
    }

    pub fn get_nearby_entities_same_kind(&mut self, target_conditions: &TargetingConditions, source: EntityId, bb: &AABB) -> Vec<EntityId> {
        let nearby = self.get_entities_of_class_same_kind(source, bb);
        nearby.into_iter().filter(|&entity| target_conditions.test(self, Some(source), entity)).collect()
    }

    pub fn get_entities_of_mob_class(&self, mob: EntityId, bb: &AABB) -> Vec<EntityId> {
        let mut entities = self.get_entities(None, bb);
        entities.retain(|&entity| self.is_instance_of_mob_class(mob, entity));
        entities
    }

    fn is_instance_of_mob_class(&self, mob: EntityId, other: EntityId) -> bool {
        match (self.find_entity(mob), self.find_entity(other)) {
            (Some(LevelEntity::Mob(a)), Some(LevelEntity::Mob(b))) => match a.kind {
                MobKind::Zombie(_) => b.is_zombie(),
                MobKind::Spider(_) => b.is_spider(),
                _ => std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind),
            },
            _ => false,
        }
    }

    fn is_same_mob_class(&self, mob: EntityId, other: EntityId) -> bool {
        match (self.find_entity(mob), self.find_entity(other)) {
            (Some(LevelEntity::Mob(a)), Some(LevelEntity::Mob(b))) => std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind),
            _ => false,
        }
    }
}
