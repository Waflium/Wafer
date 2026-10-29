use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::Codec;
use crate::world::item::component::types::{self, Pair};

const NAMES: &[&str] = &["down", "up", "north", "south", "west", "east"];

pub fn codec() -> Pair {
    types::enumeration(NAMES)
}

fn verify_vertical(direction: Nbt) -> Result<Nbt, String> {
    match direction.as_str() {
        Some("down" | "up") => Ok(direction),
        _ => Err("Expected a vertical direction".into()),
    }
}

pub fn vertical_codec() -> Pair {
    (Codec::Normalize(Box::new(Codec::Enum(NAMES)), verify_vertical), codec().1)
}
