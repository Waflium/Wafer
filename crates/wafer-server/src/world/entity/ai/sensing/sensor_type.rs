use super::mob_sensor::MobSensor;
use super::sensor::Sensor;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::animal::armadillo::armadillo;

pub const ARMADILLO_SCARE_DETECTED: MobSensor = MobSensor {
    mob_test: armadillo::is_scared_by,
    ready_test: armadillo::can_stay_rolled_up,
    to_set: memory_module_type::DANGER_DETECTED_RECENTLY,
    memory_time_to_live: 80,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SensorType {
    NearestItems,
    NearestLivingEntities,
    NearestPlayers,
    HurtBy,
    ArmadilloScareDetected,
    NearestAdult,
    AxolotlAttackables,
    FoodTemptations,
    FrogTemptations,
    FrogAttackables,
    IsInWater,
}

impl SensorType {
    pub fn name(self) -> &'static str {
        match self {
            SensorType::NearestItems => "minecraft:nearest_items",
            SensorType::NearestLivingEntities => "minecraft:nearest_living_entities",
            SensorType::NearestPlayers => "minecraft:nearest_players",
            SensorType::HurtBy => "minecraft:hurt_by",
            SensorType::ArmadilloScareDetected => "minecraft:armadillo_scare_detected",
            SensorType::NearestAdult => "minecraft:nearest_adult",
            SensorType::AxolotlAttackables => "minecraft:axolotl_attackables",
            SensorType::FoodTemptations => "minecraft:food_temptations",
            SensorType::FrogTemptations => "minecraft:frog_temptations",
            SensorType::FrogAttackables => "minecraft:frog_attackables",
            SensorType::IsInWater => "minecraft:is_in_water",
        }
    }

    pub fn create(self) -> Sensor {
        match self {
            SensorType::ArmadilloScareDetected => Sensor::with_scan_rate(self, 5),
            _ => Sensor::new(self),
        }
    }
}
