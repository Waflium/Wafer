use wafer_core::{BlockPos, Vec3};
use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::parameters::loot_context_param_sets;

#[derive(Clone, Copy)]
pub struct DefaultBlockInteractionTrigger(pub CriterionTrigger);

impl DefaultBlockInteractionTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, pos: BlockPos) {
        let state = level.block_state(pos);
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::DefaultBlockInteraction(t) = instance else { return false };
            let mut params = LootParams::new(level);
            params.origin = Some(Vec3::at_center_of(pos));
            params.this_entity = Some(player);
            params.block_state = Some(state);
            t.matches(&mut LootContextBuilder::new(params).create(None))
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    location: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let location = fields.condition("location");
            Some(TriggerInstance { player, location })
        })
    }

    pub fn matches(&self, location_context: &mut LootContext) -> bool {
        simple_criterion_trigger::matches(self.location.as_ref(), location_context)
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_optional_holder(&validator.context(&loot_context_param_sets::BLOCK_USE), "location", self.location.as_ref());
    }
}
