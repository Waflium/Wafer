use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::{self, Pair, req};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::level::storage::loot::validatable::Validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub enum ScoreboardNameProvider {
    Fixed(String),
    Context(EntityTarget),
}

fn context_target(provider: Nbt) -> Result<Nbt, String> {
    match provider.get("type").and_then(Nbt::as_str) {
        Some("minecraft:context") => Ok(provider.get("target").cloned().expect("a context target")),
        _ => Ok(provider),
    }
}

impl ScoreboardNameProvider {
    pub fn codec() -> Pair {
        let target = || types::enumeration(&["this", "attacker", "direct_attacker", "attacking_player", "target_entity", "interacting_entity"]);
        let cases = vec![("minecraft:fixed", vec![req("name", types::string())]), ("minecraft:context", vec![req("target", target())])];
        let typed = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:loot_score_provider_type"), cases }));
        (Codec::Normalize(Box::new(Codec::Either(Box::new(target().0), Box::new(typed))), context_target), Stream::Nbt)
    }

    pub fn decode(tag: &Nbt) -> DataResult<ScoreboardNameProvider> {
        match codec::either(tag, EntityTarget::decode, |t| {
            let ty = codec::decode_field(t, "type", codec::identifier)?;
            match ty.as_str() {
                "minecraft:fixed" => Ok(ScoreboardNameProvider::Fixed(codec::decode_field(t, "name", |n| Ok(codec::string(n)?.to_string()))?)),
                "minecraft:context" => Ok(ScoreboardNameProvider::Context(codec::decode_field(t, "target", EntityTarget::decode)?)),
                other => Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_score_provider_type]: {other}")),
            }
        })? {
            Ok(target) => Ok(ScoreboardNameProvider::Context(target)),
            Err(provider) => Ok(provider),
        }
    }

    pub fn get_score_holder(&self, context: &LootContext) -> Option<ScoreHolder> {
        match self {
            ScoreboardNameProvider::Fixed(name) => Some(ScoreHolder::for_name_only(name.clone())),
            ScoreboardNameProvider::Context(target) => target.get(&context.params).map(|entity| ScoreHolder::for_entity(context.params.level, entity)),
        }
    }

    pub fn score_value(&self, context: &LootContext, score: &str) -> Option<i32> {
        let score_holder = self.get_score_holder(context)?;
        let scoreboard = context.params.level.scoreboard.borrow();
        let objective = scoreboard.get_objective(score)?;
        scoreboard.get_player_score_info(score_holder.scoreboard_name(), objective.id()).map(|info| info.value)
    }
}

impl Validatable for ScoreboardNameProvider {
    fn validate(&self, context: &ValidationContext) {
        if let ScoreboardNameProvider::Context(target) = self {
            context.validate_context_usage(&[target.context_param()]);
        }
    }
}
