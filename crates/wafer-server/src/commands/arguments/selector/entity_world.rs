use std::cell::RefCell;

use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::server::level::server_player::ServerPlayer;
use crate::world::entity::EntityId;
use crate::world::level::storage::command_storage::CommandStorage;
use crate::world::scores::scoreboard::Scoreboard;

pub trait EntityWorld {
    fn levels(&self) -> Box<dyn Iterator<Item = &ServerLevel> + '_>;

    fn level(&self, index: usize) -> &ServerLevel;

    fn level_of_entity(&self, id: EntityId) -> Option<&ServerLevel>;

    fn players(&self) -> Box<dyn Iterator<Item = &ServerPlayer> + '_>;

    fn scoreboard(&self) -> &RefCell<Scoreboard>;

    fn command_storage(&self) -> &RefCell<CommandStorage>;

    fn has_player(&self, id: EntityId) -> bool {
        self.level_of_entity(id).is_some_and(|level| level.players.contains_key(&id))
    }

    fn entity_tag_to_compare(&self, id: EntityId) -> wafer_protocol::nbt::Nbt {
        let level = self.level_of_entity(id).expect("entity in a loaded level");
        level.entity_tag_to_compare_with(id, &|pearl| self.level_of_entity(pearl).filter(|level| level.entities.contains_key(&pearl)).or_else(|| level.other_levels.iter().find(|level| level.entities.contains_key(&pearl))))
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
    fn levels(&self) -> Box<dyn Iterator<Item = &ServerLevel> + '_> {
        Box::new(self.levels.iter())
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

impl LevelWorld<'_> {
    fn all_levels(&self) -> impl Iterator<Item = &ServerLevel> {
        let (before, after) = self.0.other_levels.split_at(self.0.index.min(self.0.other_levels.len()));
        before.iter().chain(std::iter::once(self.0)).chain(after.iter())
    }
}

fn holds(level: &ServerLevel, id: EntityId) -> bool {
    level.players.contains_key(&id) || level.entities.contains_key(&id) || level.dragon_parts.contains_key(id)
}

impl EntityWorld for LevelWorld<'_> {
    fn levels(&self) -> Box<dyn Iterator<Item = &ServerLevel> + '_> {
        Box::new(self.all_levels())
    }

    fn level(&self, index: usize) -> &ServerLevel {
        self.all_levels().find(|level| level.index == index).unwrap_or(self.0)
    }

    fn level_of_entity(&self, id: EntityId) -> Option<&ServerLevel> {
        self.all_levels().find(|level| holds(level, id))
    }

    fn players(&self) -> Box<dyn Iterator<Item = &ServerPlayer> + '_> {
        let mut players: Vec<&ServerPlayer> = self.all_levels().flat_map(|level| level.players.values()).collect();
        players.sort_by_key(|player| player.player_list_order);
        Box::new(players.into_iter())
    }

    fn scoreboard(&self) -> &RefCell<Scoreboard> {
        &self.0.scoreboard
    }

    fn command_storage(&self) -> &RefCell<CommandStorage> {
        &self.0.command_storage
    }
}
