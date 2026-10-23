use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::int_range_predicate::IntRangePredicate;
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;
use crate::world::level::storage::loot::validatable::Validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct EntityHasScoreCondition {
    scores: Vec<(String, IntRangePredicate)>,
    entity_target: EntityTarget,
}

impl EntityHasScoreCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<EntityHasScoreCondition> {
        let mut record = codec::Record::default();
        let scores = record.required(tag, "scores", |t| codec::unbounded_map(t, |k| Ok(k.to_string()), IntRangePredicate::decode));
        let entity_target = record.required(tag, "entity", EntityTarget::decode);
        record.build(|| Some(EntityHasScoreCondition { scores: scores?, entity_target: entity_target? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(entity) = self.entity_target.get(&context.params) else { return false };
        let holder = ScoreHolder::for_entity(context.params.level, entity);
        self.scores.iter().all(|(objective_name, range)| self.has_score(context, &holder, objective_name, range))
    }

    fn has_score(&self, context: &mut LootContext, holder: &ScoreHolder, objective_name: &str, range: &IntRangePredicate) -> bool {
        let value = {
            let scoreboard = context.params.level.scoreboard.borrow();
            let Some(objective) = scoreboard.get_objective(objective_name) else { return false };
            let Some(value) = scoreboard.get_player_score_info(holder.scoreboard_name(), objective.id()).map(|score| score.value) else { return false };
            value
        };
        range.test(context, value)
    }
}

impl EntityHasScoreCondition {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![self.entity_target.context_param()]
    }

    pub fn validate(&self, context: &ValidationContext) {
        for (score, value) in &self.scores {
            value.validate(&context.for_map_field("scores", score));
        }
    }
}
