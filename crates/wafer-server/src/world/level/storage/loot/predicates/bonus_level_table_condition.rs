use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::get_item_enchantment_level;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct BonusLevelTableCondition {
    enchantment: String,
    values: Vec<f32>,
}

impl BonusLevelTableCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<BonusLevelTableCondition> {
        let mut record = codec::Record::default();
        let enchantment = record.required(tag, "enchantment", Enchantment::decode_holder);
        let values = record.required(tag, "chances", |t| {
            codec::list_partial(t, |value| DecodeResult::from(codec::float(value))).validate(|values| if values.is_empty() { Err("List must have contents".to_string()) } else { Ok(()) })
        });
        record.build(|| Some(BonusLevelTableCondition { enchantment: enchantment?, values: values? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let level = context.params.tool.map_or(0, |tool| get_item_enchantment_level(&self.enchantment, tool));
        let chance = self.values[(level.max(0) as usize).min(self.values.len() - 1)];
        context.random().next_float() < chance
    }
}

impl BonusLevelTableCondition {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::Tool]
    }
}
