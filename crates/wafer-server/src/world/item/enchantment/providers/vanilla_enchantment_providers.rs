use super::enchantment_provider::EnchantmentProvider;
use super::enchantments_by_cost_with_difficulty::EnchantmentsByCostWithDifficulty;

pub const MOB_SPAWN_EQUIPMENT: EnchantmentProvider =
    EnchantmentProvider::ByCostWithDifficulty(EnchantmentsByCostWithDifficulty { enchantments: "minecraft:on_mob_spawn_equipment", min_cost: 5, max_cost_span: 17 });
