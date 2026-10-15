use wafer_data::registry;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VillagerType(pub i32);

impl VillagerType {
    pub const DESERT: VillagerType = VillagerType(0);
    pub const JUNGLE: VillagerType = VillagerType(1);
    pub const PLAINS: VillagerType = VillagerType(2);
    pub const SAVANNA: VillagerType = VillagerType(3);
    pub const SNOW: VillagerType = VillagerType(4);
    pub const SWAMP: VillagerType = VillagerType(5);
    pub const TAIGA: VillagerType = VillagerType(6);

    pub fn name(self) -> &'static str {
        registry::VILLAGER_TYPE[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<VillagerType> {
        registry::VILLAGER_TYPE.iter().position(|&entry| entry == name).map(|id| VillagerType(id as i32))
    }

    pub fn by_id(id: i32) -> Option<VillagerType> {
        (0..registry::VILLAGER_TYPE.len() as i32).contains(&id).then_some(VillagerType(id))
    }
}

pub fn by_biome(biome: &str) -> VillagerType {
    match biome {
        "minecraft:badlands" | "minecraft:desert" | "minecraft:eroded_badlands" | "minecraft:wooded_badlands" => VillagerType::DESERT,
        "minecraft:bamboo_jungle" | "minecraft:jungle" | "minecraft:sparse_jungle" => VillagerType::JUNGLE,
        "minecraft:savanna_plateau" | "minecraft:savanna" | "minecraft:windswept_savanna" => VillagerType::SAVANNA,
        "minecraft:deep_frozen_ocean"
        | "minecraft:frozen_ocean"
        | "minecraft:frozen_river"
        | "minecraft:ice_spikes"
        | "minecraft:snowy_beach"
        | "minecraft:snowy_taiga"
        | "minecraft:snowy_plains"
        | "minecraft:grove"
        | "minecraft:snowy_slopes"
        | "minecraft:frozen_peaks"
        | "minecraft:jagged_peaks" => VillagerType::SNOW,
        "minecraft:swamp" | "minecraft:mangrove_swamp" => VillagerType::SWAMP,
        "minecraft:old_growth_spruce_taiga"
        | "minecraft:old_growth_pine_taiga"
        | "minecraft:windswept_gravelly_hills"
        | "minecraft:windswept_hills"
        | "minecraft:taiga"
        | "minecraft:windswept_forest" => VillagerType::TAIGA,
        _ => super::villager_data::DEFAULT_TYPE,
    }
}
