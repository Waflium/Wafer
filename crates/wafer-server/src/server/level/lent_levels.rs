use super::ServerLevel;

pub struct LentLevels<'a> {
    levels: &'a mut Vec<ServerLevel>,
    index: usize,
    level: Option<ServerLevel>,
}

impl<'a> LentLevels<'a> {
    pub fn lend(levels: &'a mut Vec<ServerLevel>, index: usize) -> LentLevels<'a> {
        let mut others = std::mem::take(levels);
        let mut level = others.remove(index);
        level.other_levels = others;
        LentLevels { levels, index, level: Some(level) }
    }

    pub fn level(&mut self) -> &mut ServerLevel {
        self.level.as_mut().expect("a lent level")
    }
}

impl Drop for LentLevels<'_> {
    fn drop(&mut self) {
        if let Some(mut level) = self.level.take() {
            let mut levels = std::mem::take(&mut level.other_levels);
            levels.insert(self.index, level);
            *self.levels = levels;
        }
    }
}

impl ServerLevel {
    pub fn find_entity(&self, id: crate::world::entity::EntityId) -> Option<&crate::world::entity::LevelEntity> {
        match self.entities.get(&id) {
            Some(entity) => Some(entity),
            None => self.other_levels.iter().find_map(|level| level.entities.get(&id)),
        }
    }

    pub fn find_entity_mut(&mut self, id: crate::world::entity::EntityId) -> Option<&mut crate::world::entity::LevelEntity> {
        match self.entities.get_index_of(&id) {
            Some(index) => self.entities.get_index_mut(index).map(|(_, entity)| entity),
            None => self.other_levels.iter_mut().find_map(|level| level.entities.get_mut(&id)),
        }
    }

    pub fn find_player(&self, id: crate::world::entity::EntityId) -> Option<&crate::server::level::ServerPlayer> {
        match self.players.get(&id) {
            Some(player) => Some(player),
            None => self.other_levels.iter().find_map(|level| level.players.get(&id)),
        }
    }

    pub fn find_player_mut(&mut self, id: crate::world::entity::EntityId) -> Option<&mut crate::server::level::ServerPlayer> {
        match self.players.get_index_of(&id) {
            Some(index) => self.players.get_index_mut(index).map(|(_, player)| player),
            None => self.other_levels.iter_mut().find_map(|level| level.players.get_mut(&id)),
        }
    }

    pub fn get_entity_in_any_dimension(&self, uuid: u128) -> Option<crate::world::entity::EntityId> {
        self.entity_manager.visible_entity_storage.get(uuid).or_else(|| self.other_levels.iter().find_map(|level| level.entity_manager.visible_entity_storage.get(uuid)))
    }

    pub fn get_player_in_any_dimension(&self, uuid: u128) -> Option<crate::world::entity::EntityId> {
        std::iter::once(self).chain(&self.other_levels).find_map(|level| level.players.values().find(|player| player.entity.uuid == uuid)).map(|player| player.entity.id)
    }

    pub fn is_in_level(&self, id: crate::world::entity::EntityId) -> bool {
        self.players.contains_key(&id) || self.entities.contains_key(&id) || self.dragon_parts.contains_key(id)
    }

    pub fn level_holding(&self, id: crate::world::entity::EntityId) -> &ServerLevel {
        if self.players.contains_key(&id) || self.entities.contains_key(&id) {
            return self;
        }
        self.other_levels.iter().find(|level| level.players.contains_key(&id) || level.entities.contains_key(&id)).unwrap_or(self)
    }

    pub fn level_holding_mut(&mut self, id: crate::world::entity::EntityId) -> &mut ServerLevel {
        if self.players.contains_key(&id) || self.entities.contains_key(&id) {
            return self;
        }
        match self.other_levels.iter().position(|level| level.players.contains_key(&id) || level.entities.contains_key(&id)) {
            Some(index) => &mut self.other_levels[index],
            None => self,
        }
    }

    pub fn entity_in_any_dimension(&self, (index, id): (usize, crate::world::entity::EntityId)) -> Option<&crate::world::entity::Entity> {
        if index == self.index {
            self.entity_ref(id)
        } else {
            self.other_levels.iter().find(|level| level.index == index).and_then(|level| level.entity_ref(id))
        }
    }

    pub fn level_with_player(&self, id: crate::world::entity::EntityId) -> Option<&ServerLevel> {
        if self.players.contains_key(&id) {
            Some(self)
        } else {
            self.other_levels.iter().find(|level| level.players.contains_key(&id))
        }
    }
}
