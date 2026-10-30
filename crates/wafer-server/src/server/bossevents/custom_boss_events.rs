use std::io;
use std::path::PathBuf;

use wafer_core::random::LegacyRandomSource;
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::custom_boss_event::CustomBossEvent;
use crate::util::codec::{self, DecodeResult};
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::network::chat::Component;
use crate::server::level::ServerLevel;
use crate::util::java_collections::{JavaHashMap, identifier_hash};
use crate::world::entity::{EntityId, create_insecure_uuid};
use crate::world::level::storage::{read_saved_data_dat, write_dat};

pub struct CustomBossEvents {
    dir: Option<PathBuf>,
    events: JavaHashMap<CustomBossEvent>,
    dirty: bool,
}

impl CustomBossEvents {
    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("custom_boss_events.dat")
    }

    fn decode(data: &Nbt) -> DecodeResult<Vec<(String, CustomBossEvent)>> {
        let mut result = codec::base_map_partial(data, crate::resources::identifier::read, |packed| CustomBossEvent::load(0, String::new(), packed));
        let mut random = LegacyRandomSource::create();
        for (id, event) in result.value.iter_mut().flatten() {
            event.custom_id.clone_from(id);
            event.event.id = create_insecure_uuid(&mut random);
        }
        result
    }

    pub fn load(dir: Option<PathBuf>) -> CustomBossEvents {
        let loaded = dir.as_deref().and_then(|dir| match read_saved_data_dat(&CustomBossEvents::file(dir), DataFixTypes::SavedDataCustomBossEvents) {
            Ok(Some(tag)) => {
                let decoded = codec::with_nbt_ops(|| match tag.get("data") {
                    Some(data) => CustomBossEvents::decode(data),
                    None => DecodeResult::error("Not a map: null"),
                });
                if let Some(error) = decoded.error {
                    error!(thread: "main", "Failed to parse saved data for 'SavedDataType[minecraft:custom_boss_events]': {error}");
                }
                decoded.value
            }
            Ok(None) => None,
            Err(_) => {
                error!(thread: "main", "Error loading saved data: SavedDataType[minecraft:custom_boss_events]");
                None
            }
        });
        let mut events = JavaHashMap::new(identifier_hash);
        let dirty = loaded.as_ref().is_none_or(|entries| !entries.is_empty());
        for (id, event) in loaded.into_iter().flatten() {
            events.insert(id, event);
        }
        CustomBossEvents { dir, events, dirty }
    }

    pub fn get(&self, id: &str) -> Option<&CustomBossEvent> {
        self.events.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut CustomBossEvent> {
        self.events.get_mut(id)
    }

    pub fn create(&mut self, random: &mut LegacyRandomSource, id: &str, name: Component) -> &mut CustomBossEvent {
        let event = CustomBossEvent::new(create_insecure_uuid(random), id.to_string(), name);
        self.events.insert(id.to_string(), event);
        self.dirty = true;
        self.events.get_mut(id).unwrap()
    }

    pub fn remove(&mut self, id: &str) {
        if self.events.remove(id).is_some() {
            self.dirty = true;
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn events(&self) -> impl Iterator<Item = &CustomBossEvent> {
        self.events.iter().map(|(_, e)| e)
    }

    pub fn on_player_connect(&mut self, levels: &[ServerLevel], player: EntityId) {
        for event in self.events.values_mut() {
            event.on_player_connect(levels, player);
        }
    }

    pub fn on_player_disconnect(&mut self, player: EntityId) {
        for event in self.events.values_mut() {
            event.on_player_disconnect(player);
        }
    }

    fn is_dirty(&mut self) -> bool {
        self.dirty || self.events.iter().any(|(_, e)| e.dirty)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = self.dir.clone() else { return Ok(()) };
        if !self.is_dirty() {
            return Ok(());
        }
        let events = self.events.iter().map(|(id, e)| (id.to_string(), e.pack())).collect();
        let file = Nbt::Compound(vec![("data".into(), Nbt::Compound(events)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))].into());
        write_dat(&CustomBossEvents::file(&dir), &file)?;
        self.dirty = false;
        self.events.values_mut().for_each(|e| e.dirty = false);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_errors_match_vanilla() {
        let cases: [(&str, Option<&str>); 19] = [
            ("5", Some("Not a map: 5")),
            ("{}", None),
            ("{\"test:a\":5}", Some("Not a map: 5 missed input: {\"test:a\":5}")),
            ("{\"test:a\":{}}", Some("No key Name in MapLike[{}] missed input: {\"test:a\":{}}")),
            ("{\"test:a\":{Name:\"x\"}}", None),
            (
                "{\"test:a\":{Name:5}}",
                Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 5; Second: Not a map: 5 missed input: {\"test:a\":{Name:5}}"),
            ),
            ("{\"test:a\":{Name:\"x\",Visible:\"y\"}}", Some("Not a number missed input: {\"test:a\":{Name:\"x\",Visible:\"y\"}}")),
            ("{\"test:a\":{Name:\"x\",Visible:2}}", None),
            ("{\"test:a\":{Name:\"x\",Max:\"y\"}}", Some("Not a number missed input: {\"test:a\":{Max:\"y\",Name:\"x\"}}")),
            ("{\"test:a\":{Name:\"x\",Color:\"nope\"}}", Some("Unknown element name:nope missed input: {\"test:a\":{Color:\"nope\",Name:\"x\"}}")),
            ("{\"test:a\":{Name:\"x\",Color:5}}", Some("Not a string missed input: {\"test:a\":{Color:5,Name:\"x\"}}")),
            ("{\"test:a\":{Name:\"x\",Overlay:\"nope\"}}", Some("Unknown element name:nope missed input: {\"test:a\":{Name:\"x\",Overlay:\"nope\"}}")),
            ("{\"test:a\":{Name:\"x\",Players:5}}", Some("Not a list: 5 missed input: {\"test:a\":{Name:\"x\",Players:5}}")),
            ("{\"test:a\":{Name:\"x\",Players:[5]}}", Some("Not a list missed input: {\"test:a\":{Name:\"x\",Players:[5]}}")),
            ("{\"test:a\":{Name:\"x\",Players:[[I;1,2,3]]}}", Some("Input is not a list of 4 ints missed input: {\"test:a\":{Name:\"x\",Players:[[I;1,2,3]]}}")),
            ("{\"test:a\":{Name:\"x\",Players:[[I;1,2,3,4],[I;1,2,3,4]]}}", None),
            (
                "{\"Bad Key\":{Name:\"x\"}}",
                Some("Not a valid resource location: Bad Key Non [a-z0-9/._-] character in path of location: minecraft:Bad Key missed input: {\"Bad Key\":{Name:\"x\"}}"),
            ),
            (
                "{\"test:a\":{Name:\"x\",Visible:\"y\",Value:\"z\",Color:\"nope\"}}",
                Some("Unknown element name:nope; Not a number; Not a number missed input: {\"test:a\":{Color:\"nope\",Name:\"x\",Value:\"z\",Visible:\"y\"}}"),
            ),
            (
                "{\"test:a\":{Name:\"x\"},\"test:b\":5,\"Bad Key\":{}}",
                Some("Not a map: 5; No key Name in MapLike[{}]; Not a valid resource location: Bad Key Non [a-z0-9/._-] character in path of location: minecraft:Bad Key missed input: {\"Bad Key\":{},\"test:b\":5}"),
            ),
        ];
        for (input, expected) in cases {
            let tag = crate::nbt::parse_fully(input).ok().unwrap();
            let result = codec::with_nbt_ops(|| CustomBossEvents::decode(&tag));
            assert_eq!(result.error.as_deref(), expected, "{input}");
        }
    }

    #[test]
    fn partial_decode_keeps_what_vanilla_keeps() {
        let cases: [(&str, Option<&[&str]>); 9] = [
            ("5", None),
            ("{}", Some(&[])),
            ("{\"test:a\":5}", Some(&[])),
            ("{\"test:a\":{Name:5}}", Some(&[])),
            ("{\"test:a\":{Name:\"x\",Visible:\"y\"}}", Some(&["test:a"])),
            ("{\"test:a\":{Name:\"x\",Color:\"nope\"}}", Some(&["test:a"])),
            ("{\"test:a\":{Name:\"x\",Players:5}}", Some(&["test:a"])),
            ("{\"Bad Key\":{Name:\"x\"}}", Some(&[])),
            ("{\"test:a\":{Name:\"x\"},\"test:b\":5,\"Bad Key\":{}}", Some(&["test:a"])),
        ];
        for (input, keys) in cases {
            let tag = crate::nbt::parse_fully(input).ok().unwrap();
            let result = codec::with_nbt_ops(|| CustomBossEvents::decode(&tag));
            let decoded: Option<Vec<String>> = result.value.map(|entries| entries.into_iter().map(|(id, _)| id).collect());
            let expected: Option<Vec<String>> = keys.map(|keys| keys.iter().map(|key| key.to_string()).collect());
            assert_eq!(decoded, expected, "{input}");
        }
    }

    #[test]
    fn loaded_events_are_dirty() {
        let dir = std::env::temp_dir().join(format!("wafer-custom-boss-events-{}", std::process::id()));
        let file = CustomBossEvents::file(&dir);
        let data = crate::nbt::parse_fully("{DataVersion:5023,data:{\"test:a\":{Name:\"x\"}}}").ok().unwrap();
        write_dat(&file, &data).unwrap();
        assert!(CustomBossEvents::load(Some(dir.clone())).dirty);
        let data = crate::nbt::parse_fully("{DataVersion:5023,data:{}}").ok().unwrap();
        write_dat(&file, &data).unwrap();
        assert!(!CustomBossEvents::load(Some(dir.clone())).dirty);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
