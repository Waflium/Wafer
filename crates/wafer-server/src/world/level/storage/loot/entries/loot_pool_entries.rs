use wafer_protocol::nbt::Nbt;

use super::composite_entry_base::CompositeEntryBase;
use super::expandable_container_base::ExpandableContainerBase;
use super::loot_pool_entry_container::{LootPoolEntryContainer, LootPoolEntryContainerType};
use super::single_entry_container_base::SingleEntryContainerBase;
use crate::util::codec::{self, DataResult, DecodeResult, Record};

pub fn decode(tag: &Nbt) -> DataResult<LootPoolEntryContainer> {
    decode_partial(tag).into_result()
}

pub fn decode_partial(tag: &Nbt) -> DecodeResult<LootPoolEntryContainer> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let ty = match codec::decode_field(tag, "type", codec::identifier) {
        Ok(ty) => ty,
        Err(error) => return DecodeResult::error(error),
    };
    let mut record = Record::default();
    let decoded = match ty.as_str() {
        "minecraft:empty" | "minecraft:item" | "minecraft:dynamic" | "minecraft:slots" => {
            SingleEntryContainerBase::decode_in(&mut record, tag, &ty).map(|(entry, common)| (LootPoolEntryContainerType::Single(entry), common))
        }
        "minecraft:loot_table" | "minecraft:tag" => ExpandableContainerBase::decode_in(&mut record, tag, &ty).map(|(entry, common)| (LootPoolEntryContainerType::Expandable(entry), common)),
        "minecraft:alternatives" | "minecraft:sequence" | "minecraft:group" => {
            let (entry, common) = CompositeEntryBase::decode_in(&mut record, tag, &ty);
            Some((LootPoolEntryContainerType::Composite(entry), common))
        }
        other => return DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_pool_entry_type]: {other}")),
    };
    record.build(|| decoded.map(|(entry, common)| LootPoolEntryContainer::new(common, entry)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_error(json: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| decode(&crate::world::item::component::json_to_nbt(&value)).err().expect("a decode error"))
    }

    #[test]
    fn an_item_entry_reports_its_fields_in_vanilla_order() {
        assert_eq!(
            decode_error(r#"{"type":"minecraft:item","name":"minecraft:nope","weight":"x","quality":"y"}"#),
            r#"Not a number: "y"; Not a number: "x"; Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: minecraft:nope"#
        );
    }

    #[test]
    fn a_tag_entry_reports_the_expand_flag_after_the_tag() {
        assert_eq!(
            decode_error(r#"{"type":"minecraft:tag","name":5,"expand":"x"}"#),
            r#"Not a boolean: "x"; No key items in MapLike[{"type":"minecraft:tag","name":5,"expand":"x"}]"#
        );
    }

    #[test]
    fn a_group_reports_failing_children_before_the_common_fields() {
        let error = decode_error(r#"{"type":"minecraft:group","children":[{"type":"minecraft:nope"}],"condition":5}"#);
        assert!(error.starts_with("Not a JSON object: 5; "), "{error}");
        assert!(error.ends_with("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_pool_entry_type]: minecraft:nope"), "{error}");
    }
}
