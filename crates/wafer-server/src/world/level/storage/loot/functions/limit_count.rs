use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::int_limit::IntLimit;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct LimitCount {
    limit: IntLimit,
}

impl LimitCount {
    pub fn decode(tag: &Nbt) -> DecodeResult<LimitCount> {
        let mut record = codec::Record::default();
        let limit = record.required(tag, "limit", IntLimit::decode);
        record.build(|| Some(LimitCount { limit: limit? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        item_stack.count = self.limit.clamp(context, item_stack.count);
        item_stack
    }
}

impl LimitCount {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate(context, "limit", &self.limit);
    }
}
