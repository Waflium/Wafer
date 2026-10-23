use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub const NO_LIMIT: i32 = 0;

pub struct EnchantedCountIncreaseFunction {
    enchantment: String,
    count: Holder<ContextFloatProvider>,
    limit: i32,
}

impl EnchantedCountIncreaseFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<EnchantedCountIncreaseFunction> {
        let mut record = codec::Record::default();
        let enchantment = record.required(tag, "enchantment", Enchantment::decode_holder);
        let count = record.required(tag, "count", ContextFloatProvider::decode_holder);
        let limit = record.with_default(tag, "limit", NO_LIMIT, codec::int);
        record.build(|| Some(EnchantedCountIncreaseFunction { enchantment: enchantment?, count: count?, limit }))
    }

    fn has_limit(&self) -> bool {
        self.limit > 0
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let Some(killer) = context.params.attacking_entity.filter(|&e| context.params.level.is_living(e)) else { return item_stack };
        let level = context.params.level.get_enchantment_level(&self.enchantment, killer);
        if level == 0 {
            return item_stack;
        }
        let addition = level as f32 * self.count.value().get_float(context);
        item_stack.grow(crate::util::java_round_f(addition));
        if self.has_limit() {
            item_stack.limit_size(self.limit);
        }
        item_stack
    }
}

impl EnchantedCountIncreaseFunction {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::AttackingEntity]
    }

    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "count", &self.count);
    }
}
