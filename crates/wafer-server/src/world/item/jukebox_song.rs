use std::sync::OnceLock;

use wafer_data::SyncedRegistry;
use wafer_protocol::nbt::Nbt;

use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc};
use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::item::component::text;
use crate::world::item::component::codec::{Codec, RangeError};

const SONG_END_PADDING_TICKS: i64 = 20;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct JukeboxSong {
    pub id: i32,
    pub length_in_seconds: f32,
    pub comparator_output: i32,
}

impl JukeboxSong {
    fn decode_direct(tag: &Nbt, id: i32) -> Option<JukeboxSong> {
        Some(JukeboxSong {
            id,
            length_in_seconds: tag.get("length_in_seconds")?.as_f64()? as f32,
            comparator_output: tag.get("comparator_output")?.as_i64()? as i32,
        })
    }

    fn registered() -> &'static [(&'static str, JukeboxSong)] {
        static SONGS: OnceLock<Vec<(&'static str, JukeboxSong)>> = OnceLock::new();
        SONGS.get_or_init(|| {
            let registry = SyncedRegistry::get("minecraft:jukebox_song");
            crate::core::data_registries::json_entries("minecraft:jukebox_song")
                .iter()
                .map(|&(name, json)| {
                    let tag = dc::json_to_nbt(&serde_json::from_str(json).expect("vanilla JSON"));
                    let id = registry.id_of(name).unwrap_or(-1);
                    (name, JukeboxSong::decode_direct(&tag, id).expect("vanilla jukebox song"))
                })
                .collect()
        })
    }

    pub fn by_name(name: &str) -> Option<JukeboxSong> {
        JukeboxSong::registered().iter().find(|(n, _)| *n == name).map(|(_, song)| *song)
    }

    fn decode(tag: &Nbt) -> Option<JukeboxSong> {
        match tag.as_str() {
            Some(name) => JukeboxSong::by_name(&crate::commands::arguments::parse_identifier(name)?),
            None => JukeboxSong::decode_direct(tag, -1),
        }
    }

    pub fn length_in_ticks(&self) -> i32 {
        (self.length_in_seconds * 20.0).ceil() as i32
    }

    pub fn has_finished(&self, ticks_elapsed: i64) -> bool {
        ticks_elapsed >= self.length_in_ticks() as i64 + SONG_END_PADDING_TICKS
    }

    pub fn from_stack(stack: &ItemStack) -> Option<JukeboxSong> {
        stack.get(dc::JUKEBOX_PLAYABLE).and_then(JukeboxSong::decode)
    }
}

pub fn direct_codec() -> Pair {
    record(vec![
        req("sound_event", types::sound_event()),
        req("description", text::text()),
        req("length_in_seconds", types::float(types::positive_float())),
        req("comparator_output", types::var_int(Codec::Int(0, 15, RangeError::Within))),
    ])
}
