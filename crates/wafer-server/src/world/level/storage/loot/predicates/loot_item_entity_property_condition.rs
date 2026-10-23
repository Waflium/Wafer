use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::entity::entity_predicate::EntityPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct LootItemEntityPropertyCondition {
    predicate: Option<EntityPredicate>,
    entity_target: EntityTarget,
}

impl LootItemEntityPropertyCondition {
    pub fn decode(tag: &Nbt) -> DataResult<LootItemEntityPropertyCondition> {
        let mut record = codec::Record::default();
        let predicate = record.field(codec::decode_optional(tag, "predicate", EntityPredicate::decode));
        let entity_target = record.field(codec::decode_field(tag, "entity", EntityTarget::decode));
        record.finish()?;
        Ok(LootItemEntityPropertyCondition { predicate: predicate.unwrap(), entity_target: entity_target.unwrap() })
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
