#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MobCategory {
    Monster,
    Creature,
    Ambient,
    Axolotls,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}

impl MobCategory {
    pub const VALUES: [MobCategory; 8] = [
        MobCategory::Monster,
        MobCategory::Creature,
        MobCategory::Ambient,
        MobCategory::Axolotls,
        MobCategory::UndergroundWaterCreature,
        MobCategory::WaterCreature,
        MobCategory::WaterAmbient,
        MobCategory::Misc,
    ];

    pub fn name(self) -> &'static str {
        ["monster", "creature", "ambient", "axolotls", "underground_water_creature", "water_creature", "water_ambient", "misc"][self as usize]
    }

    pub fn by_name(name: &str) -> Option<MobCategory> {
        Self::VALUES.into_iter().find(|c| c.name() == name)
    }

    pub fn get_debug_abbreviation(self) -> &'static str {
        ["MO", "C", "AM", "AX", "UWC", "WC", "WA", "MI"][self as usize]
    }

    pub fn get_max_instances_per_chunk(self) -> i32 {
        [70, 10, 15, 5, 5, 5, 20, -1][self as usize]
    }

    pub fn is_friendly(self) -> bool {
        self != MobCategory::Monster
    }

    pub fn is_persistent(self) -> bool {
        matches!(self, MobCategory::Creature | MobCategory::Misc)
    }

    pub fn get_despawn_distance(self) -> i32 {
        if self == MobCategory::WaterAmbient { 64 } else { 128 }
    }

    pub fn get_no_despawn_distance(self) -> i32 {
        32
    }
}
