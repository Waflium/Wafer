use std::cell::RefCell;

use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::server::level::server_player::ServerPlayer;
use crate::world::entity::EntityId;
use crate::world::level::storage::command_storage::CommandStorage;
use crate::world::scores::scoreboard::Scoreboard;

pub trait EntityWorld {
    fn levels(&self) -> &[ServerLevel];

    fn level(&self, index: usize) -> &ServerLevel;

    fn level_of_entity(&self, id: EntityId) -> Option<&ServerLevel>;

    fn players(&self) -> Box<dyn Iterator<Item = &ServerPlayer> + '_>;

    fn scoreboard(&self) -> &RefCell<Scoreboard>;

    fn command_storage(&self) -> &RefCell<CommandStorage>;

    fn has_player(&self, id: EntityId) -> bool {
        self.level_of_entity(id).is_some_and(|level| level.players.contains_key(&id))
    }

    fn entity_name(&self, id: EntityId) -> String {
        let Some(level) = self.level_of_entity(id) else { return String::new() };
        match level.find_player(id) {
            Some(p) => p.profile.name.clone(),
            None => match level.find_entity(id) {
                Some(entity) => entity.name().get_string(),
                None => level.dragon_part_entity(id).map_or_else(String::new, |part| crate::world::entity::entity_name(part).get_string()),
            },
        }
    }

    fn entity_display_name(&self, id: EntityId) -> Component {
        match self.level_of_entity(id) {
            Some(level) => level.display_name(id),
            None => Component::empty(),
        }
    }
}

impl EntityWorld for Server {
    fn levels(&self) -> &[ServerLevel] {
        &self.levels
    }

    fn level(&self, index: usize) -> &ServerLevel {
        &self.levels[index]
    }

    fn level_of_entity(&self, id: EntityId) -> Option<&ServerLevel> {
        self.level_of(id).map(|level| &self.levels[level])
    }

    fn players(&self) -> Box<dyn Iterator<Item = &ServerPlayer> + '_> {
        Box::new(Server::players(self))
    }

    fn scoreboard(&self) -> &RefCell<Scoreboard> {
        &self.scoreboard
    }

    fn command_storage(&self) -> &RefCell<CommandStorage> {
        &self.command_storage
    }
}

pub struct LevelWorld<'a>(pub &'a ServerLevel);

impl EntityWorld for LevelWorld<'_> {
    fn levels(&self) -> &[ServerLevel] {
        std::slice::from_ref(self.0)
    }

    fn level(&self, _index: usize) -> &ServerLevel {
        self.0
    }

    fn level_of_entity(&self, id: EntityId) -> Option<&ServerLevel> {
        (self.0.players.contains_key(&id) || self.0.entities.contains_key(&id) || self.0.dragon_parts.contains_key(id)).then_some(self.0)
    }

    fn players(&self) -> Box<dyn Iterator<Item = &ServerPlayer> + '_> {
        Box::new(self.0.players.values())
    }

    fn scoreboard(&self) -> &RefCell<Scoreboard> {
        &self.0.scoreboard
    }

    fn command_storage(&self) -> &RefCell<CommandStorage> {
        &self.0.command_storage
    }
}
