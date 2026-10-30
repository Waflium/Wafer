use wafer_data::{Block, BlockState};
use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{self, Codec};
use crate::world::item::component::types::{self, Pair};

fn decode_state(tag: &Nbt) -> Result<BlockState, String> {
    let fields = codec::map(tag)?;
    let Some((_, id)) = fields.iter().find(|(k, v)| k == "id" && !matches!(v, Nbt::End)) else {
        return Err(format!("No key id in MapLike[{}]", crate::nbt::to_string(tag)));
    };
    let id = Codec::Element("minecraft:block").decode(id)?;
    let block = Block::by_name(id.as_str().unwrap_or("")).expect("registered block");
    let mut state = block.default_state();
    if let Some((_, Nbt::Compound(properties))) = fields.iter().find(|(k, _)| k == "properties") {
        for (name, value) in properties {
            if let Some(value) = value.as_str()
                && let Some(with) = state.with_property_by_name(name, value)
            {
                state = with;
            }
        }
    }
    Ok(state)
}

pub fn java_to_string(full: &Nbt) -> String {
    let id = full.get("id").and_then(Nbt::as_str).unwrap_or_default();
    let mut text = format!("Block{{{id}}}");
    if let Some(Nbt::Compound(properties)) = full.get("properties") {
        let mut values: Vec<(&str, &str)> = properties.iter().filter_map(|(name, value)| Some((name.as_str(), value.as_str()?))).collect();
        values.sort();
        if !values.is_empty() {
            text.push('[');
            text.push_str(&values.iter().map(|(name, value)| format!("{name}={value}")).collect::<Vec<_>>().join(","));
            text.push(']');
        }
    }
    text
}

fn decode_block(tag: &Nbt) -> Result<BlockState, String> {
    Codec::Element("minecraft:block").decode(tag).map(|id| Block::by_name(id.as_str().unwrap_or("")).expect("registered block").default_state())
}

fn encode_full(state: BlockState) -> Nbt {
    let mut fields = vec![("id".to_string(), Nbt::String(state.block().name().into()))];
    let mut properties: Vec<(String, Nbt)> = state.properties().map(|(name, value)| (name.to_string(), Nbt::String(value.into()))).collect();
    if !properties.is_empty() {
        properties.sort_by(|a, b| b.0.cmp(&a.0));
        fields.push(("properties".into(), Nbt::Compound(properties.into())));
    }
    Nbt::Compound(fields.into())
}

fn encode(state: BlockState) -> Nbt {
    let block = state.block();
    if state == block.default_state() {
        return Nbt::String(block.name().into());
    }
    encode_full(state)
}

fn decode_full(tag: Nbt) -> Result<Nbt, String> {
    decode_state(&tag).map(encode_full)
}

pub fn full_codec() -> Pair {
    types::transform(decode_full)
}

pub fn to_full(tag: &Nbt) -> Result<Nbt, String> {
    match tag {
        Nbt::String(_) => decode_block(tag).map(encode_full),
        other => decode_state(other).map(encode_full),
    }
}

fn decode(tag: Nbt) -> Result<Nbt, String> {
    match decode_block(&tag) {
        Ok(state) => Ok(encode(state)),
        Err(first) => decode_state(&tag).map(encode).map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}")),
    }
}

pub fn codec() -> Pair {
    types::transform(decode)
}
