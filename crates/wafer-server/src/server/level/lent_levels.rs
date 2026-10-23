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
