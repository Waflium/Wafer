use std::sync::LazyLock;

use wafer_core::fastutil::OpenHashMap;
use wafer_core::hash_map::FastMap;
use wafer_core::mth;
use wafer_core::random::LegacyRandomSource;
use wafer_protocol::nbt::Nbt;

use super::anger_level::AngerLevel;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::util::extra_codecs;
use crate::world::entity::entity_save::uuid_from_nbt;
use crate::world::entity::{EntityId, RemovalReason};
use crate::world::item::component::codec::Codec;
use crate::world::item::component::types::{Pair, empty_list, lenient, list, nbt, non_negative_int, record, req, var_int};
use crate::world::level::storage::uuid_ints;

pub const CONVERSION_DELAY: i32 = 2;
pub const MAX_ANGER: i32 = 150;
const DEFAULT_ANGER_DECREASE: i32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Suspect {
    entity: EntityId,
    uuid: u128,
}

pub struct AngerManagement {
    conversion_delay: i32,
    highest_anger: i32,
    suspects: Vec<Suspect>,
    anger_by_suspect: FastMap<EntityId, i32>,
    anger_by_uuid: OpenHashMap<u128, i32>,
}

impl AngerManagement {
    pub fn new(anger_by_uuid: Vec<(u128, i32)>) -> AngerManagement {
        let mut by_uuid = OpenHashMap::with_expected(anger_by_uuid.len(), 0.75);
        for (uuid, anger) in anger_by_uuid {
            by_uuid.insert(uuid, anger);
        }
        AngerManagement {
            conversion_delay: mth::random_between_inclusive(&mut LegacyRandomSource::create(), 0, CONVERSION_DELAY),
            highest_anger: 0,
            suspects: Vec::new(),
            anger_by_suspect: FastMap::default(),
            anger_by_uuid: by_uuid,
        }
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<AngerManagement> {
        static CODEC: LazyLock<Pair> = LazyLock::new(|| {
            let uuid = |tag: &Nbt| extra_codecs::uuid(tag).map(uuid_ints);
            let suspect = record(vec![req("uuid", nbt(Codec::Func(uuid))), req("anger", var_int(non_negative_int()))]);
            record(vec![lenient("suspects", list(suspect), empty_list())])
        });
        CODEC.0.decode_partial(tag).map(|decoded| AngerManagement::new(decode_suspects(decoded.get("suspects")).unwrap_or_default()))
    }

    pub fn save(&self) -> Nbt {
        let pairs = self.create_uuid_anger_pairs();
        if pairs.is_empty() {
            return Nbt::Compound(Vec::new());
        }
        let suspects = pairs.into_iter().map(|(uuid, anger)| Nbt::Compound(vec![("uuid".into(), uuid_ints(uuid)), ("anger".into(), Nbt::Int(anger))])).collect();
        Nbt::Compound(vec![("suspects".into(), Nbt::List(suspects))])
    }

    fn create_uuid_anger_pairs(&self) -> Vec<(u128, i32)> {
        self.suspects
            .iter()
            .map(|suspect| (suspect.uuid, self.anger_of(suspect.entity)))
            .chain(self.anger_by_uuid.iter().map(|(uuid, &anger)| (uuid, anger)))
            .collect()
    }

    fn anger_of(&self, entity: EntityId) -> i32 {
        self.anger_by_suspect.get(&entity).copied().unwrap_or(0)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, valid_entity: impl Fn(&ServerLevel, EntityId) -> bool) {
        self.conversion_delay -= 1;
        if self.conversion_delay <= 0 {
            self.convert_from_uuids(level);
            self.conversion_delay = CONVERSION_DELAY;
        }
        self.anger_by_uuid.retain(|_, anger| {
            if *anger <= DEFAULT_ANGER_DECREASE {
                false
            } else {
                *anger -= DEFAULT_ANGER_DECREASE;
                true
            }
        });
        let entities: Vec<EntityId> = self.anger_by_suspect.keys().copied().collect();
        for entity in entities {
            let anger = self.anger_of(entity);
            let removal_reason = removal_reason(level, entity);
            if removal_reason.is_none() && anger > 1 && valid_entity(level, entity) {
                self.anger_by_suspect.insert(entity, anger - DEFAULT_ANGER_DECREASE);
                continue;
            }
            let uuid = self.suspects.iter().find(|suspect| suspect.entity == entity).map(|suspect| suspect.uuid);
            if let Some(index) = self.suspects.iter().position(|suspect| suspect.entity == entity) {
                self.suspects.remove(index);
            }
            self.anger_by_suspect.remove(&entity);
            if anger > 1
                && matches!(removal_reason, Some(RemovalReason::ChangedDimension | RemovalReason::UnloadedToChunk | RemovalReason::UnloadedWithPlayer))
                && let Some(uuid) = uuid
            {
                self.anger_by_uuid.insert(uuid, anger - 1);
            }
        }
        self.sort_and_update_highest_anger(level);
    }

    fn sort_and_update_highest_anger(&mut self, level: &ServerLevel) {
        self.highest_anger = 0;
        if self.suspects.len() >= 2 {
            self.highest_anger = self.suspects.iter().map(|suspect| self.anger_of(suspect.entity)).max().unwrap_or(0);
        }
        let angers = &self.anger_by_suspect;
        let anger_of = |entity: EntityId| angers.get(&entity).copied().unwrap_or(0);
        self.suspects.sort_by(|suspect1, suspect2| {
            let (anger1, anger2) = (anger_of(suspect1.entity), anger_of(suspect2.entity));
            let (angry_at1, angry_at2) = (AngerLevel::by_anger(anger1).is_angry(), AngerLevel::by_anger(anger2).is_angry());
            if angry_at1 != angry_at2 {
                return if angry_at1 { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
            }
            let (is_player1, is_player2) = (level.is_player(suspect1.entity), level.is_player(suspect2.entity));
            if is_player1 != is_player2 {
                return if is_player1 { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
            }
            anger2.cmp(&anger1)
        });
        if self.suspects.len() == 1 {
            self.highest_anger = self.anger_of(self.suspects[0].entity);
        }
    }

    fn convert_from_uuids(&mut self, level: &mut ServerLevel) {
        let mut converted: Vec<(Suspect, i32)> = Vec::new();
        self.anger_by_uuid.retain(|uuid, anger| match level.entity_manager.visible_entity_storage.get(uuid) {
            Some(entity) => {
                converted.push((Suspect { entity, uuid }, *anger));
                false
            }
            None => true,
        });
        for (suspect, anger) in converted {
            self.anger_by_suspect.insert(suspect.entity, anger);
            self.suspects.push(suspect);
            level.anger_suspects.insert(suspect.entity);
        }
    }

    pub fn increase_anger(&mut self, level: &mut ServerLevel, entity: EntityId, increment: i32) -> i32 {
        let new_suspect = !self.anger_by_suspect.contains_key(&entity);
        let mut current_anger = MAX_ANGER.min(self.anger_of(entity) + increment);
        self.anger_by_suspect.insert(entity, current_anger);
        if new_suspect {
            let uuid = level.entity(entity).uuid;
            let serialized_anger = self.anger_by_uuid.remove(uuid).unwrap_or(0);
            current_anger += serialized_anger;
            self.anger_by_suspect.insert(entity, current_anger);
            self.suspects.push(Suspect { entity, uuid });
            level.anger_suspects.insert(entity);
        }
        self.sort_and_update_highest_anger(level);
        current_anger
    }

    pub fn clear_anger(&mut self, level: &ServerLevel, entity: EntityId) {
        self.anger_by_suspect.remove(&entity);
        if let Some(index) = self.suspects.iter().position(|suspect| suspect.entity == entity) {
            self.suspects.remove(index);
        }
        self.sort_and_update_highest_anger(level);
    }

    fn get_top_suspect(&self, level: &ServerLevel, filter: impl Fn(&ServerLevel, EntityId) -> bool) -> Option<EntityId> {
        self.suspects.iter().map(|suspect| suspect.entity).find(|&entity| level.entity_ref(entity).is_some() && filter(level, entity))
    }

    pub fn get_active_anger(&self, current_target: Option<EntityId>) -> i32 {
        match current_target {
            None => self.highest_anger,
            Some(target) => self.anger_of(target),
        }
    }

    pub fn get_active_entity(&self, level: &ServerLevel, filter: impl Fn(&ServerLevel, EntityId) -> bool) -> Option<EntityId> {
        self.get_top_suspect(level, filter).filter(|&entity| level.is_living(entity))
    }
}

fn removal_reason(level: &ServerLevel, entity: EntityId) -> Option<RemovalReason> {
    match level.entity_ref(entity) {
        Some(e) => e.removal,
        None => Some(level.dropped_suspect_reasons.get(&entity).map_or(RemovalReason::Discarded, |&(reason, _)| reason)),
    }
}

fn decode_suspects(suspects: Option<&Nbt>) -> Option<Vec<(u128, i32)>> {
    let Some(suspects) = suspects else { return Some(Vec::new()) };
    let Nbt::List(suspects) = suspects else { return None };
    suspects
        .iter()
        .map(|suspect| {
            let uuid = uuid_from_nbt(suspect.get("uuid")?)?;
            let anger = suspect.get("anger")?.as_i64().filter(|&anger| anger >= 0 && anger <= i32::MAX as i64)? as i32;
            Some((uuid, anger))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(snbt: &str) -> Nbt {
        crate::nbt::parse_fully(snbt).expect("valid snbt")
    }

    #[test]
    fn decoding_keeps_every_suspect_of_a_valid_list() {
        let decoded = AngerManagement::decode(&parse("{suspects:[{uuid:[I;1,2,3,4],anger:5},{uuid:[I;5,6,7,8],anger:9}]}"));
        assert!(decoded.error.is_none());
        assert_eq!(decoded.value.unwrap().anger_by_uuid.len(), 2);
    }

    #[test]
    fn a_missing_suspects_list_decodes_to_nobody() {
        let decoded = AngerManagement::decode(&parse("{}"));
        assert!(decoded.error.is_none());
        assert_eq!(decoded.value.unwrap().anger_by_uuid.len(), 0);
    }

    #[test]
    fn a_broken_suspect_makes_the_lenient_list_fall_back_to_nobody_without_an_error() {
        let decoded = AngerManagement::decode(&parse("{suspects:[{uuid:[I;1,2,3,4],anger:-1},{uuid:[I;5,6,7,8],anger:9}]}"));
        assert!(decoded.error.is_none());
        assert_eq!(decoded.value.map(|anger| anger.anger_by_uuid.len()), Some(0));
    }

    #[test]
    fn a_tag_that_is_not_a_compound_is_an_error() {
        let decoded = AngerManagement::decode(&parse("1"));
        assert!(decoded.error.is_some());
    }
}
