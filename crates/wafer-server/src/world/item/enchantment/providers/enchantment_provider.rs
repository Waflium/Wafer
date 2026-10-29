use std::sync::OnceLock;

use serde_json::Value;
use wafer_core::random::RandomSource;

use super::enchantments_by_cost::EnchantmentsByCost;
use super::enchantments_by_cost_with_difficulty::EnchantmentsByCostWithDifficulty;
use super::single_enchantment::SingleEnchantment;
use crate::core::holder_set::HolderSet;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::item::ItemStack;
use crate::util::valueproviders::int_provider_codec;
use crate::world::item::component::codec::{self, Codec, Dispatch, RangeError, Selector, Stream};
use crate::world::item::component::types::{self, Pair, req};
use crate::world::item::component::values::ItemEnchantments;

pub enum EnchantmentProvider {
    ByCost(EnchantmentsByCost),
    ByCostWithDifficulty(EnchantmentsByCostWithDifficulty),
    Single(SingleEnchantment),
}

pub fn direct_codec() -> Pair {
    let provider_cost = |name: &'static str, min: i32| req(name, (Codec::Int(min, 10000, RangeError::Within), Stream::Int));
    let cases = vec![
        ("minecraft:by_cost", vec![req("enchantments", types::holder_set("minecraft:enchantment")), req("cost", int_provider_codec())]),
        (
            "minecraft:by_cost_with_difficulty",
            vec![req("enchantments", types::holder_set("minecraft:enchantment")), provider_cost("min_cost", 1), provider_cost("max_cost_span", 0)],
        ),
        ("minecraft:single", vec![req("enchantment", types::fixed("minecraft:enchantment")), req("level", int_provider_codec())]),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:enchantment_provider_type"), cases })), Stream::Dispatch)
}

impl EnchantmentProvider {
    pub fn parse(json: &Value) -> Result<EnchantmentProvider, String> {
        let kind = json["type"].as_str().ok_or("No key type in MapLike")?;
        match crate::resources::identifier::read(kind)?.as_str() {
            "minecraft:by_cost" => Ok(EnchantmentProvider::ByCost(EnchantmentsByCost::parse(json)?)),
            "minecraft:by_cost_with_difficulty" => Ok(EnchantmentProvider::ByCostWithDifficulty(EnchantmentsByCostWithDifficulty::parse(json)?)),
            "minecraft:single" => Ok(EnchantmentProvider::Single(SingleEnchantment::parse(json)?)),
            other => Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_provider_type]: {other}")),
        }
    }

    pub fn enchant(&self, item: &ItemStack, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource, difficulty: &DifficultyInstance) {
        match self {
            EnchantmentProvider::ByCost(provider) => provider.enchant(item, item_enchantments, random),
            EnchantmentProvider::ByCostWithDifficulty(provider) => provider.enchant(item, item_enchantments, random, difficulty),
            EnchantmentProvider::Single(provider) => provider.enchant(item_enchantments, random),
        }
    }
}

pub fn get(key: &str) -> Option<&'static EnchantmentProvider> {
    static PROVIDERS: OnceLock<Vec<(&'static str, EnchantmentProvider)>> = OnceLock::new();
    PROVIDERS
        .get_or_init(|| {
            let mut providers = Vec::new();
            for &(name, json) in crate::core::data_registries::json_entries("minecraft:enchantment_provider") {
                match serde_json::from_str(json).map_err(|e| e.to_string()).and_then(|json: Value| EnchantmentProvider::parse(&json)) {
                    Ok(provider) => providers.push((name, provider)),
                    Err(e) => error!("Failed to parse enchantment provider {name}: {e}"),
                }
            }
            providers
        })
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, provider)| provider)
}

pub fn parse_enchantments(json: &Value) -> Result<HolderSet, String> {
    HolderSet::decode("minecraft:enchantment", &crate::world::item::component::json_to_nbt(json))
}

pub fn enchantment_names(enchantments: &HolderSet) -> Vec<&'static str> {
    let names = codec::registry("minecraft:enchantment");
    enchantments.contents().iter().map(|&id| names[id as usize]).collect()
}

pub fn int_range(json: &Value, min: i32, max: i32) -> Result<i32, String> {
    let value = crate::util::valueproviders::json_int(json).ok_or_else(|| format!("Not a number: {json}"))?;
    if value < min || value > max {
        return Err(format!("Value must be within range [{min};{max}]: {value}"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::super::vanilla_enchantment_providers;

    #[test]
    fn vanilla_providers_load() {
        for key in [
            vanilla_enchantment_providers::MOB_SPAWN_EQUIPMENT,
            vanilla_enchantment_providers::PILLAGER_SPAWN_CROSSBOW,
            vanilla_enchantment_providers::RAID_PILLAGER_POST_WAVE_3,
            vanilla_enchantment_providers::RAID_PILLAGER_POST_WAVE_5,
            vanilla_enchantment_providers::RAID_VINDICATOR,
            vanilla_enchantment_providers::RAID_VINDICATOR_POST_WAVE_5,
            vanilla_enchantment_providers::ENDERMAN_LOOT_DROP,
        ] {
            assert!(super::get(key).is_some(), "{key}");
        }
    }
}
