use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::types::{self, Pair, opt, record, req};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnchantmentTarget {
    Attacker,
    DamagingEntity,
    Victim,
}

impl EnchantmentTarget {
    fn decode(tag: &Nbt) -> DataResult<EnchantmentTarget> {
        codec::string_enum(tag, &[("attacker", EnchantmentTarget::Attacker), ("damaging_entity", EnchantmentTarget::DamagingEntity), ("victim", EnchantmentTarget::Victim)])
    }

    fn decode_non_damage(tag: &Nbt) -> DataResult<EnchantmentTarget> {
        codec::string_enum(tag, &[("attacker", EnchantmentTarget::Attacker), ("victim", EnchantmentTarget::Victim)])
    }
}

pub fn codec(effect: Pair) -> Pair {
    record(vec![
        req("enchanted", types::enumeration(&["attacker", "damaging_entity", "victim"])),
        req("affected", types::enumeration(&["attacker", "damaging_entity", "victim"])),
        req("effect", effect),
        opt("requirements", LootItemCondition::holder_codec()),
    ])
}

pub fn equipment_drops_codec(effect: Pair) -> Pair {
    record(vec![req("enchanted", types::enumeration(&["attacker", "victim"])), req("effect", effect), opt("requirements", LootItemCondition::holder_codec())])
}

pub struct TargetedConditionalEffect<T> {
    pub enchanted: EnchantmentTarget,
    pub affected: EnchantmentTarget,
    pub effect: T,
    pub requirements: Option<Holder<LootItemCondition>>,
}

impl<T> TargetedConditionalEffect<T> {
    pub fn decode<R: Into<DecodeResult<T>>>(tag: &Nbt, effect: impl FnOnce(&Nbt) -> R) -> DecodeResult<TargetedConditionalEffect<T>> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = codec::Record::default();
        let enchanted = record.required(tag, "enchanted", EnchantmentTarget::decode);
        let affected = record.required(tag, "affected", EnchantmentTarget::decode);
        let effect = record.required(tag, "effect", effect);
        let requirements = record.optional(tag, "requirements", LootItemCondition::decode_holder_partial);
        record.build(|| Some(TargetedConditionalEffect { enchanted: enchanted?, affected: affected?, effect: effect?, requirements }))
    }

    pub fn decode_equipment_drops<R: Into<DecodeResult<T>>>(tag: &Nbt, effect: impl FnOnce(&Nbt) -> R) -> DecodeResult<TargetedConditionalEffect<T>> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = codec::Record::default();
        let enchanted = record.required(tag, "enchanted", EnchantmentTarget::decode_non_damage);
        let effect = record.required(tag, "effect", effect);
        let requirements = record.optional(tag, "requirements", LootItemCondition::decode_holder_partial);
        record.build(|| Some(TargetedConditionalEffect { enchanted: enchanted?, affected: EnchantmentTarget::Victim, effect: effect?, requirements }))
    }

    pub fn matches(&self, context: &mut LootContext) -> bool {
        self.requirements.as_ref().is_none_or(|requirements| requirements.value().test(context))
    }
}

impl<T> Validatable for TargetedConditionalEffect<T> {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_optional_holder(context, "requirements", self.requirements.as_ref());
    }
}
