use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::entity::entity_predicate::EntityPredicate;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct LootItemEntityPropertyCondition {
    predicate: Option<EntityPredicate>,
    entity_target: EntityTarget,
}

impl LootItemEntityPropertyCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<LootItemEntityPropertyCondition> {
        let mut record = codec::Record::default();
        let predicate = record.optional(tag, "predicate", EntityPredicate::decode);
        let entity_target = record.required(tag, "entity", EntityTarget::decode);
        record.build(|| Some(LootItemEntityPropertyCondition { predicate, entity_target: entity_target? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(predicate) = &self.predicate else { return true };
        let entity = self.entity_target.get(&context.params);
        let pos = context.params.origin;
        predicate.matches(context.level(), pos, entity)
    }
}

impl LootItemEntityPropertyCondition {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::Origin, self.entity_target.context_param()]
    }
}
