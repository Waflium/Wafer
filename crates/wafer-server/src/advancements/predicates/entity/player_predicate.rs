use wafer_core::AABB;
use wafer_protocol::nbt::Nbt;

use super::entity_predicate::{self, EntityPredicate};
use crate::advancements::advancement_progress::AdvancementProgress;
use crate::server::server_advancement_manager::ServerAdvancementManager;
use crate::advancements::predicates::food_predicate::{self, FoodPredicate};
use crate::advancements::predicates::game_type_predicate::{self, GameTypePredicate};
use crate::advancements::predicates::input_predicate::{self, InputPredicate};
use crate::advancements::predicates::min_max_bounds::{Ints, ints_codec};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::{self, Pair, def, opt, record, req};
use crate::server::level::ServerLevel;
use crate::stats::stat::Stat;
use crate::stats::stat_type::StatType;
use crate::stats::stats_counter::StatsCounter;
use crate::util::codec::{self, DataResult, DecodeError};
use crate::world::entity::EntityId;

pub const LOOKING_AT_RANGE: f64 = 100.0;

fn stat_matcher_codec() -> Pair {
    let case = |registry: &'static str| vec![req("stat", types::by_name(registry)), def("value", ints_codec(), types::empty_map())];
    let cases = vec![
        ("minecraft:mined", case("minecraft:block")),
        ("minecraft:crafted", case("minecraft:item")),
        ("minecraft:used", case("minecraft:item")),
        ("minecraft:broken", case("minecraft:item")),
        ("minecraft:picked_up", case("minecraft:item")),
        ("minecraft:dropped", case("minecraft:item")),
        ("minecraft:killed", case("minecraft:entity_type")),
        ("minecraft:killed_by", case("minecraft:entity_type")),
        ("minecraft:custom", case("minecraft:custom_stat")),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:stat_type"), cases })), Stream::Dispatch)
}

fn advancement_predicate_codec() -> Pair {
    (Codec::Either(Box::new(Codec::Bool), Box::new(Codec::Map(Box::new(Codec::string()), Box::new(Codec::Bool)))), Stream::Nbt)
}

pub fn codec() -> Pair {
    record(vec![
        def("level", ints_codec(), types::empty_map()),
        def("food", food_predicate::codec(), types::empty_map()),
        def("gamemode", game_type_predicate::codec(), types::empty_list()),
        def("stats", types::list(stat_matcher_codec()), types::empty_list()),
        def("recipes", (Codec::Map(Box::new(Codec::Identifier), Box::new(Codec::Bool)), Stream::Nbt), types::empty_map()),
        def("advancements", (Codec::Map(Box::new(Codec::Identifier), Box::new(advancement_predicate_codec().0)), Stream::Nbt), types::empty_map()),
        opt("looking_at", entity_predicate::codec()),
        opt("input", input_predicate::codec()),
    ])
}

#[derive(Debug)]
struct StatMatcher {
    stat: Stat,
    range: Ints,
}

impl StatMatcher {
    fn decode(tag: &Nbt) -> DataResult<StatMatcher> {
        let stat_type = codec::decode_field(tag, "type", |t| {
            let id = codec::identifier(t)?;
            crate::world::item::component::codec::registry_id("minecraft:stat_type", &id).map_err(|_| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:stat_type]: {id}"))
        })?;
        let stat_type = StatType::by_id(stat_type).expect("a stat type");
        let mut record = codec::Record::default();
        let value = record.field(codec::decode_field(tag, "stat", |t| {
            let id = codec::identifier(t)?;
            crate::world::item::component::codec::registry_id(stat_type.registry_key(), &id)
                .map_err(|_| format!("Unknown registry key in ResourceKey[minecraft:root / {}]: {id}", stat_type.registry_key()))
        }));
        let range = record.field(codec::decode_or(tag, "value", Ints::ANY, Ints::decode));
        record.finish()?;
        Ok(StatMatcher { stat: stat_type.get(value.unwrap() as u16), range: range.unwrap() })
    }

    fn matches(&self, counter: &impl StatsCounter) -> bool {
        self.range.matches(counter.get_value(self.stat))
    }
}

#[derive(Debug)]
enum AdvancementPredicate {
    Done(bool),
    Criterions(Vec<(String, bool)>),
}

impl AdvancementPredicate {
    fn decode(tag: &Nbt) -> DataResult<AdvancementPredicate> {
        let criterions = |t: &Nbt| {
            codec::map(t).map_err(|message| DecodeError { message, partial: false })?;
            codec::base_map(t, |k| Ok(k.to_string()), codec::bool_value).map_err(|message| DecodeError { message, partial: true })
        };
        match codec::either_or_partial(tag, codec::bool_value, criterions)? {
            Ok(done) => Ok(AdvancementPredicate::Done(done)),
            Err(criterions) => Ok(AdvancementPredicate::Criterions(criterions)),
        }
    }

    fn test(&self, progress: &AdvancementProgress) -> bool {
        match self {
            AdvancementPredicate::Done(state) => progress.is_done() == *state,
            AdvancementPredicate::Criterions(criterions) => {
                criterions.iter().all(|(name, done)| progress.get_criterion(name).is_some_and(|criterion| criterion.is_done() == *done))
            }
        }
    }
}

#[derive(Debug)]
pub struct PlayerPredicate {
    level: Ints,
    food: FoodPredicate,
    game_type: GameTypePredicate,
    stats: Vec<StatMatcher>,
    recipes: Vec<(String, bool)>,
    advancements: Vec<(String, AdvancementPredicate)>,
    looking_at: Option<Box<EntityPredicate>>,
    input: Option<InputPredicate>,
}

impl PlayerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PlayerPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let level = record.field(codec::decode_or(tag, "level", Ints::ANY, Ints::decode));
        let food = record.field(codec::decode_or(tag, "food", FoodPredicate::ANY, FoodPredicate::decode));
        let game_type = record.field(codec::decode_or(tag, "gamemode", GameTypePredicate::any(), GameTypePredicate::decode));
        let stats = record.field(codec::decode_or(tag, "stats", Vec::new(), |t| codec::list_of(t, StatMatcher::decode)));
        let recipes = record.field(codec::decode_or(tag, "recipes", Vec::new(), |t| codec::base_map(t, crate::resources::identifier::read, codec::bool_value)));
        let advancements = record.field(codec::decode_or(tag, "advancements", Vec::new(), |t| {
            codec::base_map(t, crate::resources::identifier::read, AdvancementPredicate::decode)
        }));
        let looking_at = record.field(codec::decode_optional(tag, "looking_at", |t| Ok(Box::new(EntityPredicate::decode(t)?))));
        let input = record.field(codec::decode_optional(tag, "input", InputPredicate::decode));
        record.finish()?;
        Ok(PlayerPredicate {
            level: level.unwrap(),
            food: food.unwrap(),
            game_type: game_type.unwrap(),
            stats: stats.unwrap(),
            recipes: recipes.unwrap(),
            advancements: advancements.unwrap(),
            looking_at: looking_at.unwrap(),
            input: input.unwrap(),
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        let Some(player) = level.find_player(entity) else { return false };
        if !self.level.matches(player.experience.level) {
            return false;
        }
        if !self.food.matches(&player.food_data) {
            return false;
        }
        if !self.game_type.matches(player.game_mode.game_type) {
            return false;
        }
        if !self.stats.iter().all(|stat| stat.matches(&player.stats)) {
            return false;
        }
        if self.recipes.iter().any(|(recipe, present)| player.recipe_book.contains(recipe) != *present) {
            return false;
        }
        if !self.advancements.is_empty() {
            let server_advancements = ServerAdvancementManager::get_bound();
            for (id, predicate) in &self.advancements {
                let Some(advancement) = server_advancements.get(id) else { return false };
                if !predicate.test(level.player_mut(entity).advancements.get_or_start_progress(advancement)) {
                    return false;
                }
            }
        }
        let player = &level.players[&entity];
        let last_client_input = player.last_client_input;
        if let Some(looking_at) = &self.looking_at {
            let from = level.entity(entity).eye_position();
            let view_vec = level.entity(entity).view_vector();
            let to = from.add(view_vec.x * LOOKING_AT_RANGE, view_vec.y * LOOKING_AT_RANGE, view_vec.z * LOOKING_AT_RANGE);
            let area = AABB::from_corners(from, to).inflate_all(1.0);
            let Some((looking_at_entity, _)) = level.get_entity_hit_result(entity, from, to, &area, |level, e| !level.is_spectator(e), 0.0) else {
                return false;
            };
            let position = level.entity(entity).position();
            if !looking_at.matches(level, Some(position), Some(looking_at_entity)) || !level.has_line_of_sight(entity, looking_at_entity) {
                return false;
            }
        }
        self.input.as_ref().is_none_or(|input| input.matches(last_client_input))
    }
}
