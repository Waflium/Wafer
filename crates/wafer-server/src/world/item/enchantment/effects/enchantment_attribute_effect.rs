use std::borrow::Cow;

use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation};
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::item::enchantment::level_based_value::{self, LevelBasedValue};

pub fn fields() -> Vec<Field> {
    vec![
        req("id", types::identifier()),
        req("attribute", types::by_name("minecraft:attribute")),
        req("amount", level_based_value::codec()),
        req("operation", types::enumeration(&["add_value", "add_multiplied_base", "add_multiplied_total"])),
    ]
}

pub fn map_codec() -> Pair {
    record(fields())
}

#[derive(Debug)]
pub struct EnchantmentAttributeEffect {
    pub id: String,
    pub attribute: Attribute,
    pub amount: LevelBasedValue,
    pub operation: Operation,
}

impl EnchantmentAttributeEffect {
    pub fn decode_partial(tag: &Nbt) -> DecodeResult<EnchantmentAttributeEffect> {
        static CODEC: std::sync::LazyLock<Pair> = std::sync::LazyLock::new(map_codec);
        codec::checked(&CODEC.0, tag, EnchantmentAttributeEffect::decode)
    }

    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentAttributeEffect> {
        Ok(EnchantmentAttributeEffect {
            id: codec::decode_field(tag, "id", codec::identifier)?,
            attribute: codec::decode_field(tag, "attribute", |t| {
                let id = codec::identifier(t)?;
                Attribute::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:attribute]: {id}"))
            })?,
            amount: codec::decode_field(tag, "amount", LevelBasedValue::decode)?,
            operation: codec::decode_field(tag, "operation", |t| {
                let name = codec::string(t)?;
                Operation::by_name(name).ok_or_else(|| format!("Unknown element name:{name}"))
            })?,
        })
    }

    fn id_for_slot(&self, slot: &str) -> String {
        format!("{}/{slot}", self.id)
    }

    pub fn get_modifier(&self, level: i32, slot: &str) -> AttributeModifier {
        AttributeModifier { id: Cow::Owned(self.id_for_slot(slot)), amount: self.amount.calculate(level) as f64, operation: self.operation }
    }
}
