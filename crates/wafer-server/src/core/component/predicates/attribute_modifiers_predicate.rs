use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate;
use crate::advancements::predicates::min_max_bounds::doubles_codec;
use crate::world::item::component::types::{self, Pair, def, opt, record};

fn entry_codec() -> Pair {
    record(vec![
        opt("attribute", types::holder_set("minecraft:attribute")),
        opt("id", types::identifier()),
        def("amount", doubles_codec(), types::empty_map()),
        opt("operation", types::enumeration(&["add_value", "add_multiplied_base", "add_multiplied_total"])),
        opt("slot", types::enumeration(types::EQUIPMENT_SLOT_GROUPS)),
    ])
}

pub fn codec() -> Pair {
    record(vec![opt("modifiers", collection_predicate::codec(entry_codec))])
}

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::{self as component_codec, registry_id};

const OPERATIONS: &[(&str, &str)] = &[("add_value", "add_value"), ("add_multiplied_base", "add_multiplied_base"), ("add_multiplied_total", "add_multiplied_total")];
const SLOT_GROUPS: &[(&str, &str)] = &[
    ("any", "any"),
    ("mainhand", "mainhand"),
    ("offhand", "offhand"),
    ("hand", "hand"),
    ("feet", "feet"),
    ("legs", "legs"),
    ("chest", "chest"),
    ("head", "head"),
    ("armor", "armor"),
    ("body", "body"),
    ("saddle", "saddle"),
];

#[derive(Debug)]
pub struct EntryPredicate {
    attribute: Option<HolderSet>,
    id: Option<String>,
    amount: Doubles,
    operation: Option<&'static str>,
    slot: Option<&'static str>,
}

impl EntryPredicate {
    fn decode(tag: &Nbt) -> DataResult<EntryPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let attribute = record.optional(tag, "attribute", |t| HolderSet::decode_partial("minecraft:attribute", t));
            let id = record.optional(tag, "id", codec::identifier);
            let amount = record.with_default(tag, "amount", Doubles::ANY, Doubles::decode_partial);
            let operation = record.optional(tag, "operation", |t| codec::string_enum(t, OPERATIONS));
            let slot = record.optional(tag, "slot", |t| codec::string_enum(t, SLOT_GROUPS));
            record.build(|| Some(EntryPredicate { attribute, id, amount, operation, slot }))
        })
        .into_result()
    }

    fn test(&self, value: &Nbt) -> bool {
        if let Some(attribute) = &self.attribute {
            let id = value.get("type").and_then(Nbt::as_str).and_then(|t| registry_id("minecraft:attribute", t).ok());
            if !id.is_some_and(|id| attribute.contains(id)) {
                return false;
            }
        }
        if self.id.as_ref().is_some_and(|id| value.get("id").and_then(Nbt::as_str) != Some(id.as_str())) {
            return false;
        }
        if !self.amount.matches(value.get("amount").map_or(0.0, component_codec::as_f64)) {
            return false;
        }
        if self.operation.is_some_and(|operation| value.get("operation").and_then(Nbt::as_str) != Some(operation)) {
            return false;
        }
        self.slot.is_none_or(|slot| value.get("slot").and_then(Nbt::as_str).unwrap_or("any") == slot)
    }
}

#[derive(Debug)]
pub struct AttributeModifiersPredicate {
    modifiers: Option<CollectionPredicate<EntryPredicate>>,
}

impl AttributeModifiersPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<AttributeModifiersPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let modifiers = record.optional(tag, "modifiers", |t| CollectionPredicate::decode(t, EntryPredicate::decode));
            record.build(|| Some(AttributeModifiersPredicate { modifiers }))
        })
        .into_result()
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        self.modifiers.as_ref().is_none_or(|modifiers| modifiers.test(value.as_list(), |p, entry| p.test(entry)))
    }
}
