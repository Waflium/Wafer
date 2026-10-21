use super::enchantment_provider::EnchantmentProvider;
use super::enchantments_by_cost_with_difficulty::EnchantmentsByCostWithDifficulty;
use super::single_enchantment::SingleEnchantment;
use crate::util::valueproviders::IntProvider;

pub const MOB_SPAWN_EQUIPMENT: EnchantmentProvider =
    EnchantmentProvider::ByCostWithDifficulty(EnchantmentsByCostWithDifficulty { enchantments: "minecraft:on_mob_spawn_equipment", min_cost: 5, max_cost_span: 17 });
pub const PILLAGER_SPAWN_CROSSBOW: EnchantmentProvider = EnchantmentProvider::Single(SingleEnchantment { enchantment: "minecraft:piercing", level: IntProvider::Constant(1) });
pub const ENDERMAN_LOOT_DROP: EnchantmentProvider = EnchantmentProvider::Single(SingleEnchantment { enchantment: "minecraft:silk_touch", level: IntProvider::Constant(1) });
