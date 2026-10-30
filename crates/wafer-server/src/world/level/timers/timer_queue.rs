use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;
use indexmap::IndexMap;

use super::TimerCallback;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::server::Server;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::level::storage::{read_saved_data, write_dat};

struct Event {
    trigger_time: i64,
    id: String,
    callback: TimerCallback,
}

pub struct TimerQueue {
    dir: Option<PathBuf>,
    queue: BTreeMap<(i64, u64), Event>,
    sequential_id: u64,
    events: IndexMap<String, IndexMap<i64, (i64, u64)>>,
    dirty: bool,
}

impl TimerQueue {
    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("scheduled_events.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> TimerQueue {
        let mut queue = TimerQueue { dir, queue: BTreeMap::new(), sequential_id: 0, events: IndexMap::new(), dirty: false };
        let Some(dir) = queue.dir.clone() else { return queue };
        match read_saved_data(&TimerQueue::file(&dir), "main", "scheduled_events", DataFixTypes::SavedDataScheduledEvents, TimerQueue::decode) {
            Some(events) => events.into_iter().for_each(|(time, id, callback)| queue.schedule(&id, time, callback)),
            None => queue.dirty = true,
        }
        queue
    }

    fn decode_event(tag: &Nbt) -> DecodeResult<(i64, String, TimerCallback)> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let trigger_time = record.required(tag, "trigger_time", codec::long);
        let id = record.required(tag, "id", |id| codec::string(id).map(str::to_string));
        let callback = record.required(tag, "callback", TimerCallback::decode);
        record.build(|| Some((trigger_time?, id?, callback?)))
    }

    fn decode(data: &Nbt) -> DecodeResult<Vec<(i64, String, TimerCallback)>> {
        if let Err(error) = codec::map(data) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let events = record.required(data, "events", |events| codec::list_partial(events, TimerQueue::decode_event));
        record.build(|| events)
    }

    pub fn tick(server: &mut Server, current_tick: i64) {
        loop {
            let queue = &mut server.scheduled_events;
            let Some(entry) = queue.queue.first_entry() else { return };
            if entry.get().trigger_time > current_tick {
                return;
            }
            let event = entry.remove();
            queue.remove_cell(&event.id, current_tick);
            queue.dirty = true;
            event.callback.handle(server);
        }
    }

    fn remove_cell(&mut self, id: &str, time: i64) {
        if let Some(row) = self.events.get_mut(id) {
            row.shift_remove(&time);
            if row.is_empty() {
                self.events.shift_remove(id);
            }
        }
    }

    pub fn schedule(&mut self, id: &str, time: i64, callback: TimerCallback) {
        if self.events.get(id).is_some_and(|row| row.contains_key(&time)) {
            return;
        }
        self.sequential_id += 1;
        let key = (time, self.sequential_id);
        self.events.entry(id.to_string()).or_default().insert(time, key);
        self.queue.insert(key, Event { trigger_time: time, id: id.to_string(), callback });
        self.dirty = true;
    }

    pub fn remove(&mut self, id: &str) -> i32 {
        let removed = self.events.shift_remove(id).unwrap_or_default();
        for key in removed.values() {
            self.queue.remove(key);
        }
        self.dirty = true;
        removed.len() as i32
    }

    pub fn events_ids(&self) -> impl Iterator<Item = &str> {
        self.events.keys().map(String::as_str)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let events = self
            .queue
            .values()
            .map(|event| {
                Nbt::Compound(vec![
                    ("trigger_time".into(), Nbt::Long(event.trigger_time)),
                    ("callback".into(), event.callback.to_nbt()),
                    ("id".into(), Nbt::String(event.id.clone())),
                ].into())
            })
            .collect();
        let data = Nbt::Compound(vec![("events".into(), Nbt::List(events))].into());
        let file = Nbt::Compound(vec![("data".into(), data), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))].into());
        write_dat(&TimerQueue::file(dir), &file)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_errors_match_vanilla() {
        let ok = r#"{trigger_time:1L,id:"a",callback:{type:"minecraft:function",id:"x:y"}}"#;
        let cases: Vec<(String, Option<&str>, Option<usize>)> = vec![
            ("{}".into(), Some("No key events in MapLike[{}]"), None),
            ("{events:5}".into(), Some("Not a list: 5"), None),
            ("{events:[]}".into(), None, Some(0)),
            ("{events:[5]}".into(), Some("Not a map: 5"), Some(0)),
            ("{events:[{}]}".into(), Some("No key callback in MapLike[{}]; No key id in MapLike[{}]; No key trigger_time in MapLike[{}]"), Some(0)),
            (format!("{{events:[{ok}]}}"), None, Some(1)),
            (format!(r#"{{events:[{ok},{{trigger_time:"q",id:"b",callback:{{type:"minecraft:function",id:"x:y"}}}}]}}"#), Some("Not a number"), Some(1)),
            (r#"{events:[{trigger_time:1L,id:"a",callback:{type:"minecraft:bad",id:"x:y"}}]}"#.into(), Some("Unknown element id: minecraft:bad"), Some(0)),
            (r#"{events:[{trigger_time:1L,id:"a",callback:{type:"minecraft:function"}}]}"#.into(), Some(r#"No key id in MapLike[{type:"minecraft:function"}]"#), Some(0)),
            (r#"{events:[{trigger_time:1L,id:"a",callback:{id:"x:y"}}]}"#.into(), Some(r#"No key type in MapLike[{id:"x:y"}]"#), Some(0)),
            (r#"{events:[{trigger_time:1L,id:5,callback:{type:"minecraft:function_tag",id:"x:y"}}]}"#.into(), Some("Not a string"), Some(0)),
            (format!(r#"{{events:[{ok},5,{{trigger_time:2L,id:"b",callback:{{type:"minecraft:function",id:"x:y"}}}}]}}"#), Some("Not a map: 5"), Some(2)),
            (r#"{events:[{trigger_time:1L,id:"a",callback:5}]}"#.into(), Some("Not a map: 5"), Some(0)),
            (
                r#"{events:[{trigger_time:1L,id:"a",callback:{type:"minecraft:function",id:"BAD"}}]}"#.into(),
                Some("Not a valid resource location: BAD Non [a-z0-9/._-] character in path of location: minecraft:BAD"),
                Some(0),
            ),
            (r#"{events:[{trigger_time:1L,id:"a",callback:{type:5,id:"x:y"}}]}"#.into(), Some("Not a string"), Some(0)),
        ];
        for (input, error, count) in cases {
            let tag = crate::nbt::parse_fully(&input).ok().unwrap();
            let result = codec::with_nbt_ops(|| TimerQueue::decode(&tag));
            assert_eq!(result.error.as_deref(), error, "{input}");
            assert_eq!(result.value.map(|events| events.len()), count, "{input}");
        }
    }
}
