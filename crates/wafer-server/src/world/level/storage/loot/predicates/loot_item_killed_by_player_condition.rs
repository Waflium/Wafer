use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct LootItemKilledByPlayerCondition;

impl LootItemKilledByPlayerCondition {
    pub fn test(&self, context: &mut LootContext) -> bool {
        context.has_parameter(LootContextParam::LastDamagePlayer)
    }
}

impl LootItemKilledByPlayerCondition {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::LastDamagePlayer]
    }
}
