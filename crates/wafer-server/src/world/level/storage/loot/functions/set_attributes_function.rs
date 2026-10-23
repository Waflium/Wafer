use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::entity::attributes::{Attribute, Operation};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

const EQUIPMENT_SLOT_GROUPS: [&str; 11] = ["any", "mainhand", "offhand", "hand", "feet", "legs", "chest", "head", "armor", "body", "saddle"];

struct Modifier {
    id: String,
    attribute: Attribute,
    operation: Operation,
    amount: Holder<ContextFloatProvider>,
    slots: Vec<&'static str>,
}

impl Modifier {
    fn decode(tag: &Nbt) -> DecodeResult<Modifier> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = codec::Record::default();
        let id = record.required(tag, "id", codec::identifier);
        let attribute = record.required(tag, "attribute", |t| {
            let id = codec::identifier(t)?;
            Attribute::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:attribute]: {id}"))
        });
        let operation = record.required(tag, "operation", |t| {
            let name = codec::string(t)?;
            Operation::by_name(name).ok_or_else(|| format!("Unknown element name:{name}"))
        });
        let amount = record.required(tag, "amount", ContextFloatProvider::decode_holder);
        let slots = record.required(tag, "slot", |t| {
            codec::compact_list_partial(t, |s| {
                DecodeResult::from(
                    codec::string(s).and_then(|name| EQUIPMENT_SLOT_GROUPS.into_iter().find(|&group| group == name).ok_or_else(|| format!("Unknown element name:{name}"))),
                )
            })
            .validate(|slots| if slots.is_empty() { Err("List must have contents".to_string()) } else { Ok(()) })
        });
        record.build(|| Some(Modifier { id: id?, attribute: attribute?, operation: operation?, amount: amount?, slots: slots? }))
    }
}

pub struct SetAttributesFunction {
    modifiers: Vec<Modifier>,
    replace: bool,
}

impl SetAttributesFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetAttributesFunction> {
        let mut record = codec::Record::default();
        let modifiers = record.required(tag, "modifiers", |t| codec::list_partial(t, Modifier::decode));
        let replace = record.with_default(tag, "replace", true, codec::bool_value);
        record.build(|| Some(SetAttributesFunction { modifiers: modifiers?, replace }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let item_modifiers = if self.replace { Vec::new() } else { item_stack.get(dc::ATTRIBUTE_MODIFIERS).map_or(Vec::new(), |m| m.as_list().to_vec()) };
        let value = Nbt::List(self.update_modifiers(context, item_modifiers));
        let value = dc::ATTRIBUTE_MODIFIERS.decode(&value).unwrap_or(value);
        item_stack.set(dc::ATTRIBUTE_MODIFIERS, Some(value));
        item_stack
    }

    fn update_modifiers(&self, context: &mut LootContext, mut item_modifiers: Vec<Nbt>) -> Vec<Nbt> {
        for modifier in &self.modifiers {
            let slot = modifier.slots[context.random().next_int_bounded(modifier.slots.len() as i32) as usize];
            let amount = modifier.amount.value().get_float(context);
            let attribute = modifier.attribute.name();
            item_modifiers.retain(|entry| {
                let same_attribute = entry.get("type").and_then(Nbt::as_str) == Some(attribute);
                let same_id = entry.get("id").and_then(Nbt::as_str) == Some(modifier.id.as_str());
                !(same_attribute && same_id)
            });
            item_modifiers.push(Nbt::Compound(vec![
                ("type".into(), Nbt::String(attribute.to_string())),
                ("id".into(), Nbt::String(modifier.id.clone())),
                ("amount".into(), Nbt::Double(amount as f64)),
                ("operation".into(), Nbt::String(modifier.operation.name().to_string())),
                ("slot".into(), Nbt::String(slot.to_string())),
            ]));
        }
        item_modifiers
    }
}

impl SetAttributesFunction {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_list(context, "modifiers", &self.modifiers);
    }
}

impl Validatable for Modifier {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "amount", &self.amount);
    }
}
