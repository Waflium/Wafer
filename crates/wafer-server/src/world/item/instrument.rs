use std::sync::OnceLock;

use wafer_protocol::nbt::Nbt;

use crate::sounds::sound_event::SoundEvent;
use crate::world::item::component::{self as dc};

#[derive(Clone, Debug)]
pub struct Instrument {
    pub sound_event: SoundEvent,
    pub use_duration: f32,
    pub range: f32,
    pub durability_damage: i32,
}

impl Instrument {
    fn decode_direct(tag: &Nbt) -> Option<Instrument> {
        Some(Instrument {
            sound_event: SoundEvent::from_nbt(tag.get("sound_event")?)?,
            use_duration: tag.get("use_duration")?.as_f64()? as f32,
            range: tag.get("range")?.as_f64()? as f32,
            durability_damage: tag.get("durability_damage").map_or(Some(0), |v| v.as_i64().map(|v| v as i32))?,
        })
    }

    fn registered() -> &'static [(&'static str, Instrument)] {
        static INSTRUMENTS: OnceLock<Vec<(&'static str, Instrument)>> = OnceLock::new();
        INSTRUMENTS.get_or_init(|| {
            crate::core::data_registries::json_entries("minecraft:instrument")
                .iter()
                .map(|&(name, json)| {
                    let tag = dc::json_to_nbt(&serde_json::from_str(json).expect("vanilla JSON"));
                    (name, Instrument::decode_direct(&tag).expect("vanilla instrument"))
                })
                .collect()
        })
    }

    pub fn decode(tag: &Nbt) -> Option<Instrument> {
        match tag.as_str() {
            Some(name) => {
                let name = crate::commands::arguments::parse_identifier(name)?;
                Instrument::registered().iter().find(|(n, _)| *n == name).map(|(_, instrument)| instrument.clone())
            }
            None => Instrument::decode_direct(tag),
        }
    }
}
