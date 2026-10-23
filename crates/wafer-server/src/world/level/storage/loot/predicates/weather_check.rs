use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct WeatherCheck {
    is_raining: Option<bool>,
    is_thundering: Option<bool>,
}

impl WeatherCheck {
    pub fn decode(tag: &Nbt) -> DecodeResult<WeatherCheck> {
        let mut record = codec::Record::default();
        let is_raining = record.optional(tag, "raining", codec::bool_value);
        let is_thundering = record.optional(tag, "thundering", codec::bool_value);
        record.build(|| Some(WeatherCheck { is_raining, is_thundering }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let level = context.level();
        if self.is_raining.is_some_and(|raining| raining != level.is_raining()) {
            return false;
        }
        self.is_thundering.is_none_or(|thundering| thundering == level.is_thundering())
    }
}
