use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct UsedEnderEyeTrigger(pub CriterionTrigger);

impl UsedEnderEyeTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, feature: BlockPos) {
        let position = level.entity(player).position();
        let xd = position.x - feature.x as f64;
        let zd = position.z - feature.z as f64;
        let sqr_dist = xd * xd + zd * zd;
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::UsedEnderEye(t) = instance else { return false };
            t.matches(sqr_dist)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    distance: Doubles,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let distance = fields.with_default("distance", Doubles::ANY, Doubles::decode);
            Some(TriggerInstance { player, distance })
        })
    }

    pub fn matches(&self, sqr_distance: f64) -> bool {
        self.distance.matches_sqr(sqr_distance)
    }
}
