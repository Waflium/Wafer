use std::cell::OnceCell;

use wafer_core::BlockPos;

use super::{ServerLevel, ServerPlayer};
use crate::network::configuration::ClientInfo;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::Mob;
use crate::world::entity::{Entity, EntityId, LevelEntity, RemovalReason};

const TOMBSTONE_TICKS: i64 = 600;

pub struct EntityTombstones {
    removed: crate::util::FastIndexMap<EntityId, (LevelEntity, i64)>,
    placeholder: Box<(Entity, LivingEntity)>,
    player_placeholder: OnceCell<Box<ServerPlayer>>,
}

impl EntityTombstones {
    pub fn new() -> EntityTombstones {
        EntityTombstones {
            removed: crate::util::FastIndexMap::default(),
            placeholder: Box::new((Entity::removed_placeholder(), LivingEntity::player())),
            player_placeholder: OnceCell::new(),
        }
    }
}

fn removed_player() -> Box<ServerPlayer> {
    let mut player = ServerPlayer::new(Profile { uuid: 0, name: String::new(), properties: Vec::new() }, ClientInfo::default(), Outbound::detached(), BlockPos::ZERO);
    player.entity.removal = Some(RemovalReason::Discarded);
    player.health = 0.0;
    Box::new(player)
}

impl ServerLevel {
    pub(crate) fn bury_entity(&mut self, id: EntityId, entity: LevelEntity) {
        let game_time = self.game_time;
        self.tombstones.removed.insert(id, (entity, game_time));
    }

    pub(crate) fn purge_tombstones(&mut self) {
        let game_time = self.game_time;
        self.tombstones.removed.retain(|_, (_, buried)| game_time - *buried <= TOMBSTONE_TICKS);
    }

    pub(crate) fn tombstone(&self, id: EntityId) -> Option<&LevelEntity> {
        match self.tombstones.removed.get(&id) {
            Some((entity, _)) => Some(entity),
            None => self.other_levels.iter().find_map(|level| level.tombstones.removed.get(&id).map(|(entity, _)| entity)),
        }
    }

    pub(crate) fn tombstone_mut(&mut self, id: EntityId) -> Option<&mut LevelEntity> {
        match self.tombstones.removed.get_index_of(&id) {
            Some(index) => self.tombstones.removed.get_index_mut(index).map(|(_, (entity, _))| entity),
            None => self.other_levels.iter_mut().find_map(|level| level.tombstones.removed.get_mut(&id).map(|(entity, _)| entity)),
        }
    }

    pub(crate) fn find_entity_or_tombstone(&self, id: EntityId) -> Option<&LevelEntity> {
        self.find_entity(id).or_else(|| self.tombstone(id))
    }

    pub(crate) fn find_entity_or_tombstone_mut(&mut self, id: EntityId) -> Option<&mut LevelEntity> {
        if self.find_entity(id).is_some() {
            return self.find_entity_mut(id);
        }
        self.tombstone_mut(id)
    }

    pub(crate) fn entity_ref_or_tombstone(&self, id: EntityId) -> Option<&Entity> {
        self.entity_ref(id).or_else(|| self.tombstone(id).map(LevelEntity::entity))
    }

    pub(crate) fn mob_or_tombstone(&self, id: EntityId) -> Option<&Mob> {
        match self.find_entity_or_tombstone(id) {
            Some(LevelEntity::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    pub(crate) fn mob_or_tombstone_mut(&mut self, id: EntityId) -> Option<&mut Mob> {
        match self.find_entity_or_tombstone_mut(id) {
            Some(LevelEntity::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    pub(crate) fn removed_placeholder(&self) -> &(Entity, LivingEntity) {
        &self.tombstones.placeholder
    }

    pub(crate) fn removed_placeholder_mut(&mut self) -> &mut (Entity, LivingEntity) {
        &mut self.tombstones.placeholder
    }

    pub(crate) fn removed_player_placeholder(&self) -> &ServerPlayer {
        self.tombstones.player_placeholder.get_or_init(removed_player)
    }

    pub(crate) fn removed_player_placeholder_mut(&mut self) -> &mut ServerPlayer {
        self.tombstones.player_placeholder.get_or_init(removed_player);
        self.tombstones.player_placeholder.get_mut().expect("a removed player")
    }
}
