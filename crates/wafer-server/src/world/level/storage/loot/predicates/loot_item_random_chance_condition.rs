use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;

pub struct LootItemRandomChanceCondition {
    chance: Holder<ContextFloatProvider>,
}

impl LootItemRandomChanceCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<LootItemRandomChanceCondition> {
        let mut record = codec::Record::default();
        let chance = record.required(tag, "chance", ContextFloatProvider::decode_holder);
        record.build(|| Some(LootItemRandomChanceCondition { chance: chance? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let probability = self.chance.value().get_float(context);
        context.random().next_float() < probability
    }
}
