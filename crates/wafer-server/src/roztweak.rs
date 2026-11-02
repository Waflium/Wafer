use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use wafer_protocol::nbt::{Nbt, TagMap};

use crate::util::datafix::data_fix_types::current_version;
use crate::world::item::ItemStack;
use crate::world::level::chunk::storage::{decode_compressed_nbt, encode_nbt};

pub const BUKKIT_VALUES: &str = "BukkitValues";
pub const ENDER_CHEST_EXTRA: &str = "roztweak:ender_chest_extra";

pub fn decode_ender_chest_extra(bukkit_values: &Nbt) -> Vec<ItemStack> {
    let Some(Nbt::ByteArray(bytes)) = bukkit_values.get(ENDER_CHEST_EXTRA) else { return Vec::new() };
    let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
    let Ok(root) = decode_compressed_nbt(&bytes) else { return Vec::new() };
    let Some(Nbt::List(items)) = root.get("items") else { return Vec::new() };
    items.iter().map(|entry| ItemStack::from_nbt_partial(entry).value.unwrap_or_else(ItemStack::empty)).collect()
}

pub fn encode_ender_chest_extra(items: &[ItemStack]) -> Nbt {
    let list = items.iter().map(|stack| if stack.is_empty() { Nbt::Compound(TagMap::default()) } else { stack.to_nbt() }).collect();
    let root = Nbt::Compound(vec![("items".to_string(), Nbt::List(list)), ("DataVersion".to_string(), Nbt::Int(current_version()))].into());
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&encode_nbt(&root, &[])).expect("in-memory gzip");
    Nbt::ByteArray(encoder.finish().expect("in-memory gzip").into_iter().map(|b| b as i8).collect())
}

pub fn save_bukkit_values(bukkit_values: &Option<Nbt>, extra: &[ItemStack]) -> Option<Nbt> {
    let mut fields = match bukkit_values {
        Some(Nbt::Compound(fields)) => fields.clone(),
        _ => TagMap::default(),
    };
    let had_extra = fields.remove_key(ENDER_CHEST_EXTRA).is_some();
    if had_extra || extra.iter().any(|stack| !stack.is_empty()) {
        fields.push((ENDER_CHEST_EXTRA.to_string(), encode_ender_chest_extra(extra)));
    }
    (!fields.is_empty()).then_some(Nbt::Compound(fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ender_chest_extra_round_trips_through_bukkit_values() {
        let mut extra = vec![ItemStack::empty(); 27];
        extra[0] = ItemStack::new(wafer_data::items::DIAMOND, 64);
        extra[26] = ItemStack::new(wafer_data::items::ELYTRA, 1);
        let other = ("other:key".to_string(), Nbt::Int(7));
        let values = Some(Nbt::Compound(vec![other.clone()].into()));
        let saved = save_bukkit_values(&values, &extra).unwrap();
        assert_eq!(saved.get("other:key"), Some(&Nbt::Int(7)));
        let decoded = decode_ender_chest_extra(&saved);
        assert_eq!(decoded.len(), 27);
        assert_eq!(decoded[0].to_nbt(), extra[0].to_nbt());
        assert_eq!(decoded[26].to_nbt(), extra[26].to_nbt());
        assert!(decoded[1].is_empty());
    }

    #[test]
    fn empty_extra_without_saved_data_writes_nothing() {
        assert!(save_bukkit_values(&None, &vec![ItemStack::empty(); 27]).is_none());
    }
}
