use std::sync::LazyLock;

use wafer_protocol::nbt::Nbt;

use super::all_of_condition::AllOfCondition;
use super::any_of_condition::AnyOfCondition;
use super::bonus_level_table_condition::BonusLevelTableCondition;
use super::damage_source_condition::DamageSourceCondition;
use super::enchantment_active_check::EnchantmentActiveCheck;
use super::entity_has_score_condition::EntityHasScoreCondition;
use super::environment_attribute_check::EnvironmentAttributeCheck;
use super::explosion_condition::ExplosionCondition;
use super::float_value_check::FloatValueCheck;
use super::int_value_check::IntValueCheck;
use super::inverted_loot_item_condition::InvertedLootItemCondition;
use super::location_check::LocationCheck;
use super::loot_item_entity_property_condition::LootItemEntityPropertyCondition;
use super::loot_item_killed_by_player_condition::LootItemKilledByPlayerCondition;
use super::loot_item_random_chance_condition::LootItemRandomChanceCondition;
use super::loot_item_random_chance_with_enchanted_bonus_condition::LootItemRandomChanceWithEnchantedBonusCondition;
use super::match_block::MatchBlock;
use super::match_tool::MatchTool;
use super::time_check::TimeCheck;
use super::weather_check::WeatherCheck;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::advancements::predicates::damage_source_predicate;
use crate::advancements::predicates::entity::entity_predicate;
use crate::advancements::predicates::location_predicate;
use crate::world::attribute::AttributeType;
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::custom;
use crate::world::item::component::types::{self, Pair, def, opt, req};
use crate::world::item::enchantment::level_based_value;
use crate::world::level::storage::loot::float_range_predicate;
use crate::world::level::storage::loot::int_range_predicate;
use crate::world::level::storage::loot::providers::number::{floats, ints};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable::Validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub enum LootItemCondition {
    Inverted(InvertedLootItemCondition),
    AnyOf(AnyOfCondition),
    AllOf(AllOfCondition),
    RandomChance(LootItemRandomChanceCondition),
    RandomChanceWithEnchantedBonus(LootItemRandomChanceWithEnchantedBonusCondition),
    EntityProperties(LootItemEntityPropertyCondition),
    KilledByPlayer(LootItemKilledByPlayerCondition),
    EntityScores(EntityHasScoreCondition),
    MatchBlock(MatchBlock),
    MatchTool(MatchTool),
    TableBonus(BonusLevelTableCondition),
    SurvivesExplosion(ExplosionCondition),
    DamageSourceProperties(DamageSourceCondition),
    LocationCheck(LocationCheck),
    WeatherCheck(WeatherCheck),
    TimeCheck(TimeCheck),
    IntValueCheck(IntValueCheck),
    FloatValueCheck(FloatValueCheck),
    EnchantmentActiveCheck(EnchantmentActiveCheck),
    EnvironmentAttributeCheck(EnvironmentAttributeCheck),
}

impl RegistryElement for LootItemCondition {
    const REGISTRY: &'static str = "minecraft:predicate";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static LootItemCondition> {
        reloadable_server_registries::get().predicates.get(key)
    }
}

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(LootItemCondition::direct_codec);
    &CODEC
}

fn raw() -> Pair {
    (Codec::AnyTag, Stream::Nbt)
}

fn normalize_environment_attribute_check(condition: Nbt) -> Result<Nbt, String> {
    if condition.get("type").and_then(Nbt::as_str) != Some("minecraft:environment_attribute_check") {
        return Ok(condition);
    }
    let attribute = condition.get("attribute").and_then(Nbt::as_str).and_then(EnvironmentAttributeInfo::by_name).expect("a decoded attribute");
    let value = condition.get("value").expect("a decoded value");
    let decoded = attribute.ty.condition_value_codec().0.decode(value)?;
    let parsed = attribute.parse_value(&codec::to_json(if attribute.ty == AttributeType::MobSpawnSettings { &decoded } else { value }))?;
    let encoded = match attribute.ty {
        AttributeType::RgbColor | AttributeType::ArgbColor => attribute.ty.encode_value(&parsed)?,
        _ => decoded,
    };
    let Nbt::Compound(fields) = condition else { unreachable!("a condition is a compound") };
    Ok(Nbt::Compound(fields.into_iter().map(|(key, field)| if key == "value" { (key, encoded.clone()) } else { (key, field) }).collect()))
}

fn entity_target() -> Pair {
    types::enumeration(&["this", "attacker", "direct_attacker", "attacking_player", "target_entity", "interacting_entity"])
}

impl LootItemCondition {
    pub fn direct_codec() -> Pair {
        let term = |name: &'static str| req(name, Self::holder_codec());
        let terms = || req("terms", types::compact_list(Self::holder_codec()));
        let cases = vec![
            ("minecraft:inverted", vec![term("term")]),
            ("minecraft:any_of", vec![terms()]),
            ("minecraft:all_of", vec![terms()]),
            ("minecraft:random_chance", vec![req("chance", floats::holder_codec())]),
            (
                "minecraft:random_chance_with_enchanted_bonus",
                vec![
                    req("unenchanted_chance", types::float(types::codec_float_range(0.0, 1.0))),
                    req("enchanted_chance", level_based_value::codec()),
                    req("enchantment", types::fixed("minecraft:enchantment")),
                ],
            ),
            ("minecraft:entity_properties", vec![opt("predicate", entity_predicate::codec()), req("entity", entity_target())]),
            ("minecraft:killed_by_player", vec![]),
            (
                "minecraft:entity_scores",
                vec![req("scores", (Codec::Map(Box::new(Codec::string()), Box::new(int_range_predicate::codec().0)), Stream::Nbt)), req("entity", entity_target())],
            ),
            (
                "minecraft:match_block",
                match custom::block_predicate().0 {
                    Codec::Record(fields) => fields,
                    _ => unreachable!("a block predicate is a record"),
                },
            ),
            ("minecraft:match_tool", vec![opt("predicate", (custom::item_predicate(), Stream::Nbt))]),
            (
                "minecraft:table_bonus",
                vec![req("enchantment", types::fixed("minecraft:enchantment")), req("chances", types::list(types::float(Codec::float())))],
            ),
            ("minecraft:survives_explosion", vec![]),
            ("minecraft:damage_source_properties", vec![opt("predicate", damage_source_predicate::codec())]),
            (
                "minecraft:location_check",
                vec![
                    opt("predicate", location_predicate::codec()),
                    def("offsetX", (Codec::int(), Stream::Int), types::i(0)),
                    def("offsetY", (Codec::int(), Stream::Int), types::i(0)),
                    def("offsetZ", (Codec::int(), Stream::Int), types::i(0)),
                ],
            ),
            ("minecraft:weather_check", vec![opt("raining", types::bool()), opt("thundering", types::bool())]),
            (
                "minecraft:time_check",
                vec![req("clock", types::fixed("minecraft:world_clock")), opt("period", (Codec::long(), Stream::Long)), req("value", int_range_predicate::codec())],
            ),
            ("minecraft:int_value_check", vec![req("value", ints::holder_codec()), req("test", int_range_predicate::codec())]),
            ("minecraft:float_value_check", vec![req("value", floats::holder_codec()), req("test", float_range_predicate::codec())]),
            ("minecraft:enchantment_active_check", vec![req("active", types::bool())]),
            ("minecraft:environment_attribute_check", vec![req("attribute", types::by_name("minecraft:environment_attribute")), req("value", raw())]),
        ];
        let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:loot_condition_type"), cases }));
        (Codec::Normalize(Box::new(dispatch), normalize_environment_attribute_check), Stream::Nbt)
    }

    pub fn holder_codec() -> Pair {
        (Codec::Either(Box::new(Codec::Identifier), Box::new(Codec::Lazy(direct_codec_lazy))), Stream::Nbt)
    }

    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<LootItemCondition>> {
        LootItemCondition::decode_holder_partial(tag).into_result()
    }

    pub fn decode_holder_partial(tag: &Nbt) -> DecodeResult<Holder<LootItemCondition>> {
        Holder::decode_partial(tag, LootItemCondition::decode_partial)
    }

    pub fn decode_holder_list(tag: &Nbt) -> DataResult<Vec<Holder<LootItemCondition>>> {
        LootItemCondition::decode_holder_list_partial(tag).into_result()
    }

    pub fn decode_holder_list_partial(tag: &Nbt) -> DecodeResult<Vec<Holder<LootItemCondition>>> {
        codec::holder_set_partial(tag, Self::REGISTRY, |t| Holder::decode_partial(t, LootItemCondition::decode_partial))
    }

    pub fn decode(tag: &Nbt) -> DataResult<LootItemCondition> {
        LootItemCondition::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<LootItemCondition> {
        let ty = match codec::decode_field(tag, "type", codec::identifier) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        match ty.as_str() {
            "minecraft:inverted" => InvertedLootItemCondition::decode(tag).map(LootItemCondition::Inverted),
            "minecraft:any_of" => AnyOfCondition::decode(tag).map(LootItemCondition::AnyOf),
            "minecraft:all_of" => AllOfCondition::decode(tag).map(LootItemCondition::AllOf),
            "minecraft:random_chance" => LootItemRandomChanceCondition::decode(tag).map(LootItemCondition::RandomChance),
            "minecraft:random_chance_with_enchanted_bonus" => {
                LootItemRandomChanceWithEnchantedBonusCondition::decode(tag).map(LootItemCondition::RandomChanceWithEnchantedBonus)
            }
            "minecraft:entity_properties" => LootItemEntityPropertyCondition::decode(tag).map(LootItemCondition::EntityProperties),
            "minecraft:killed_by_player" => DecodeResult::success(LootItemCondition::KilledByPlayer(LootItemKilledByPlayerCondition)),
            "minecraft:entity_scores" => EntityHasScoreCondition::decode(tag).map(LootItemCondition::EntityScores),
            "minecraft:match_block" => DecodeResult::from(MatchBlock::decode(tag)).map(LootItemCondition::MatchBlock),
            "minecraft:match_tool" => MatchTool::decode(tag).map(LootItemCondition::MatchTool),
            "minecraft:table_bonus" => BonusLevelTableCondition::decode(tag).map(LootItemCondition::TableBonus),
            "minecraft:survives_explosion" => DecodeResult::success(LootItemCondition::SurvivesExplosion(ExplosionCondition)),
            "minecraft:damage_source_properties" => DamageSourceCondition::decode(tag).map(LootItemCondition::DamageSourceProperties),
            "minecraft:location_check" => LocationCheck::decode(tag).map(LootItemCondition::LocationCheck),
            "minecraft:weather_check" => WeatherCheck::decode(tag).map(LootItemCondition::WeatherCheck),
            "minecraft:time_check" => TimeCheck::decode(tag).map(LootItemCondition::TimeCheck),
            "minecraft:int_value_check" => IntValueCheck::decode(tag).map(LootItemCondition::IntValueCheck),
            "minecraft:float_value_check" => FloatValueCheck::decode(tag).map(LootItemCondition::FloatValueCheck),
            "minecraft:enchantment_active_check" => EnchantmentActiveCheck::decode(tag).map(LootItemCondition::EnchantmentActiveCheck),
            "minecraft:environment_attribute_check" => DecodeResult::from(EnvironmentAttributeCheck::decode(tag)).map(LootItemCondition::EnvironmentAttributeCheck),
            other => DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_condition_type]: {other}")),
        }
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        match self {
            LootItemCondition::Inverted(c) => c.test(context),
            LootItemCondition::AnyOf(c) => c.test(context),
            LootItemCondition::AllOf(c) => c.test(context),
            LootItemCondition::RandomChance(c) => c.test(context),
            LootItemCondition::RandomChanceWithEnchantedBonus(c) => c.test(context),
            LootItemCondition::EntityProperties(c) => c.test(context),
            LootItemCondition::KilledByPlayer(c) => c.test(context),
            LootItemCondition::EntityScores(c) => c.test(context),
            LootItemCondition::MatchBlock(c) => c.test(context),
            LootItemCondition::MatchTool(c) => c.test(context),
            LootItemCondition::TableBonus(c) => c.test(context),
            LootItemCondition::SurvivesExplosion(c) => c.test(context),
            LootItemCondition::DamageSourceProperties(c) => c.test(context),
            LootItemCondition::LocationCheck(c) => c.test(context),
            LootItemCondition::WeatherCheck(c) => c.test(context),
            LootItemCondition::TimeCheck(c) => c.test(context),
            LootItemCondition::IntValueCheck(c) => c.test(context),
            LootItemCondition::FloatValueCheck(c) => c.test(context),
            LootItemCondition::EnchantmentActiveCheck(c) => c.test(context),
            LootItemCondition::EnvironmentAttributeCheck(c) => c.test(context),
        }
    }
}

impl LootItemCondition {
    fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        match self {
            LootItemCondition::TableBonus(condition) => condition.get_referenced_context_params(),
            LootItemCondition::DamageSourceProperties(condition) => condition.get_referenced_context_params(),
            LootItemCondition::EnchantmentActiveCheck(condition) => condition.get_referenced_context_params(),
            LootItemCondition::EntityScores(condition) => condition.get_referenced_context_params(),
            LootItemCondition::EnvironmentAttributeCheck(condition) => condition.get_referenced_context_params(),
            LootItemCondition::SurvivesExplosion(condition) => condition.get_referenced_context_params(),
            LootItemCondition::LocationCheck(condition) => condition.get_referenced_context_params(),
            LootItemCondition::EntityProperties(condition) => condition.get_referenced_context_params(),
            LootItemCondition::KilledByPlayer(condition) => condition.get_referenced_context_params(),
            LootItemCondition::RandomChanceWithEnchantedBonus(condition) => condition.get_referenced_context_params(),
            LootItemCondition::MatchBlock(condition) => condition.get_referenced_context_params(),
            LootItemCondition::MatchTool(condition) => condition.get_referenced_context_params(),
            _ => Vec::new(),
        }
    }
}

impl Validatable for LootItemCondition {
    fn validate(&self, context: &ValidationContext) {
        context.validate_context_usage(&self.get_referenced_context_params());
        match self {
            LootItemCondition::AllOf(condition) => condition.validate(context),
            LootItemCondition::AnyOf(condition) => condition.validate(context),
            LootItemCondition::EntityScores(condition) => condition.validate(context),
            LootItemCondition::FloatValueCheck(condition) => condition.validate(context),
            LootItemCondition::IntValueCheck(condition) => condition.validate(context),
            LootItemCondition::Inverted(condition) => condition.validate(context),
            LootItemCondition::TimeCheck(condition) => condition.validate(context),
            _ => {}
        }
    }
}
