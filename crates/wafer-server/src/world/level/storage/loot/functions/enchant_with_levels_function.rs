use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec::registry;
use crate::world::item::enchantment::enchantment_helper::enchant_item;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct EnchantWithLevelsFunction {
    levels: Holder<ContextIntProvider>,
    options: Option<HolderSet>,
    include_additional_cost_component: bool,
}

impl EnchantWithLevelsFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<EnchantWithLevelsFunction> {
        let mut record = codec::Record::default();
        let levels = record.required(tag, "levels", ContextIntProvider::decode_holder);
        let options = record.optional(tag, "options", |t| HolderSet::decode_partial("minecraft:enchantment", t));
        let include_additional_cost_component = record.with_default(tag, "include_additional_cost_component", false, codec::bool_value);
        record.build(|| Some(EnchantWithLevelsFunction { levels: levels?, options, include_additional_cost_component }))
    }

    pub fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let enchantment_cost = self.levels.value().get_int(context);
        let names = registry("minecraft:enchantment");
        let source: Vec<&str> = match &self.options {
            Some(options) => options.contents().iter().map(|&id| names[id as usize]).collect(),
            None => crate::core::data_registries::names_in_registry_order("minecraft:enchantment"),
        };
        let mut result = enchant_item(&mut context.random(),item_stack, enchantment_cost, &source);
        if self.include_additional_cost_component && context.has_parameter(LootContextParam::AdditionalCostComponentAllowed) && !result.is_empty() && enchantment_cost > 0 {
            result.set(dc::ADDITIONAL_TRADE_COST, Some(Nbt::Int(enchantment_cost)));
        }
        result
    }
}

impl EnchantWithLevelsFunction {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        if self.include_additional_cost_component { vec![LootContextParam::AdditionalCostComponentAllowed] } else { Vec::new() }
    }

    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "levels", &self.levels);
    }
}
