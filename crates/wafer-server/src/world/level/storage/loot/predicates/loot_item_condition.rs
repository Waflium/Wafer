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
use crate::util::codec::{self, DataResult, DecodeResult};
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

impl LootItemCondition {
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
