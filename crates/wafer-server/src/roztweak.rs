use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use wafer_protocol::nbt::{Nbt, TagMap};

use crate::util::datafix::data_fix_types::current_version;
use crate::world::item::ItemStack;
use crate::world::level::chunk::storage::{decode_compressed_nbt, encode_nbt};

pub const BUKKIT_VALUES: &str = "BukkitValues";
pub const ENDER_CHEST_EXTRA: &str = "roztweak:ender_chest_extra";
const ITEMS_FORMAT_VERSION: u8 = 1;

pub fn decode_ender_chest_extra(bukkit_values: &Nbt) -> Vec<ItemStack> {
    let Some(Nbt::ByteArray(bytes)) = bukkit_values.get(ENDER_CHEST_EXTRA) else { return Vec::new() };
    let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
    deserialize_items_from_bytes(&bytes).unwrap_or_default()
}

fn deserialize_items_from_bytes(bytes: &[u8]) -> Option<Vec<ItemStack>> {
    let (&version, mut rest) = bytes.split_first()?;
    if version != ITEMS_FORMAT_VERSION {
        return None;
    }
    let count = read_int(&mut rest)?;
    let mut items = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count {
        let length = read_int(&mut rest)? as usize;
        if length == 0 {
            items.push(ItemStack::empty());
            continue;
        }
        let (item_bytes, tail) = rest.split_at_checked(length)?;
        rest = tail;
        items.push(deserialize_item(item_bytes));
    }
    Some(items)
}

fn read_int(bytes: &mut &[u8]) -> Option<i32> {
    let (head, tail) = bytes.split_first_chunk::<4>()?;
    *bytes = tail;
    Some(i32::from_be_bytes(*head))
}

fn deserialize_item(bytes: &[u8]) -> ItemStack {
    let Ok(Nbt::Compound(mut fields)) = decode_compressed_nbt(bytes) else { return ItemStack::empty() };
    fields.remove_key("DataVersion");
    ItemStack::from_nbt_partial(&Nbt::Compound(fields)).value.unwrap_or_else(ItemStack::empty)
}

pub fn encode_ender_chest_extra(items: &[ItemStack]) -> Nbt {
    let mut out = vec![ITEMS_FORMAT_VERSION];
    out.extend_from_slice(&(items.len() as i32).to_be_bytes());
    for stack in items {
        if stack.is_empty() {
            out.extend_from_slice(&0i32.to_be_bytes());
            continue;
        }
        let item_bytes = serialize_item(stack);
        out.extend_from_slice(&(item_bytes.len() as i32).to_be_bytes());
        out.extend_from_slice(&item_bytes);
    }
    Nbt::ByteArray(out.into_iter().map(|b| b as i8).collect())
}

fn serialize_item(stack: &ItemStack) -> Vec<u8> {
    let mut fields = match stack.to_nbt() {
        Nbt::Compound(fields) => fields,
        _ => TagMap::default(),
    };
    fields.push(("DataVersion".to_string(), Nbt::Int(current_version())));
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&encode_nbt(&Nbt::Compound(fields), &[])).expect("in-memory gzip");
    encoder.finish().expect("in-memory gzip")
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

    const PAPER_EXTRA_PREFIX: [u8; 9] = [0x01, 0x00, 0x00, 0x00, 0x1b, 0x00, 0x00, 0x00, 0x4f];

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
    fn the_layout_matches_paper_serialize_items_as_bytes() {
        let mut extra = vec![ItemStack::empty(); 27];
        extra[0] = ItemStack::new(wafer_data::items::DIAMOND, 64);
        let Nbt::ByteArray(bytes) = encode_ender_chest_extra(&extra) else { panic!() };
        let bytes: Vec<u8> = bytes.into_iter().map(|b| b as u8).collect();
        assert_eq!(&bytes[..5], &PAPER_EXTRA_PREFIX[..5]);
        assert_eq!(&bytes[9..11], &[0x1f, 0x8b]);
        assert_eq!(&bytes[bytes.len() - 26 * 4..], &[0u8; 26 * 4][..]);
    }

    #[test]
    #[ignore]
    fn real_paper_bytes_round_trip() {
        let input = std::fs::read(std::env::var("ROZTWEAK_EXTRA").unwrap()).unwrap();
        let values = Nbt::Compound(vec![(ENDER_CHEST_EXTRA.to_string(), Nbt::ByteArray(input.iter().map(|&b| b as i8).collect()))].into());
        let decoded = decode_ender_chest_extra(&values);
        let mut report = String::new();
        for (slot, stack) in decoded.iter().enumerate().filter(|(_, stack)| !stack.is_empty()) {
            report.push_str(&format!("[{slot}] {:?}\n", stack.to_nbt()));
        }
        std::fs::write(std::env::var("ROZTWEAK_REPORT").unwrap(), report).unwrap();
        let Nbt::ByteArray(out) = encode_ender_chest_extra(&decoded) else { panic!() };
        std::fs::write(std::env::var("ROZTWEAK_OUT").unwrap(), out.iter().map(|&b| b as u8).collect::<Vec<u8>>()).unwrap();
    }

    #[test]
    fn empty_extra_without_saved_data_writes_nothing() {
        assert!(save_bukkit_values(&None, &vec![ItemStack::empty(); 27]).is_none());
    }
}
