use std::sync::LazyLock;

use serde_json::Value;
use wafer_data::SyncedRegistry;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::{Codec, RangeError};
use crate::world::item::component::text;
use crate::world::item::component::types::{self, Pair, opt, record, req};

pub const REGISTRY: &str = "minecraft:painting_variant";
pub const PLACEABLE: &str = "minecraft:placeable";

pub fn direct_codec() -> Pair {
    let size = || types::var_int(Codec::Int(1, 16, RangeError::Within));
    record(vec![req("width", size()), req("height", size()), req("asset_id", types::identifier()), opt("title", text::text()), opt("author", text::text())])
}

pub struct PaintingVariant {
    pub width: i32,
    pub height: i32,
}

impl PaintingVariant {
    pub fn decode(tag: &Nbt) -> DataResult<PaintingVariant> {
        let width = codec::decode_field(tag, "width", |tag| codec::int_range(tag, 1, 16));
        let height = codec::decode_field(tag, "height", |tag| codec::int_range(tag, 1, 16));
        let asset_id = codec::decode_field(tag, "asset_id", codec::identifier);
        codec::record_errors(&[width.as_ref().err(), height.as_ref().err(), asset_id.as_ref().err()])?;
        Ok(PaintingVariant { width: width?, height: height? })
    }

    pub fn area(&self) -> i32 {
        self.width * self.height
    }
}

static VARIANTS: LazyLock<Vec<PaintingVariant>> = LazyLock::new(|| {
    SyncedRegistry::get(REGISTRY)
        .entries
        .iter()
        .map(|name| {
            let json = crate::core::data_registries::json_entries("minecraft:painting_variant").iter().find(|(id, _)| id == name).map(|(_, json)| *json).expect("vanilla painting variant");
            let json: Value = serde_json::from_str(json).expect("vanilla JSON");
            PaintingVariant { width: json["width"].as_i64().unwrap_or(1) as i32, height: json["height"].as_i64().unwrap_or(1) as i32 }
        })
        .collect()
});

pub fn get(id: i32) -> &'static PaintingVariant {
    &VARIANTS[id as usize]
}
