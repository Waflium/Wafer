use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::damage_source_predicate::DamageSourcePredicate;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct DamageSourceCondition {
    predicate: Option<DamageSourcePredicate>,
}

impl DamageSourceCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<DamageSourceCondition> {
        let mut record = codec::Record::default();
        let predicate = record.optional(tag, "predicate", DamageSourcePredicate::decode);
        record.build(|| Some(DamageSourceCondition { predicate }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let (Some(damage_source), Some(pos)) = (context.params.damage_source, context.params.origin) else { return false };
        self.predicate.as_ref().is_none_or(|predicate| predicate.matches(context.level(), Some(pos), &damage_source))
    }
}

impl DamageSourceCondition {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::Origin, LootContextParam::DamageSource]
    }
}
