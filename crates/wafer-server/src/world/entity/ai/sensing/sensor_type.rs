use super::sensor::Sensor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SensorType {
    NearestItems,
    NearestLivingEntities,
    NearestPlayers,
    HurtBy,
    NearestAdult,
    FoodTemptations,
}

impl SensorType {
    pub fn name(self) -> &'static str {
        match self {
            SensorType::NearestItems => "minecraft:nearest_items",
            SensorType::NearestLivingEntities => "minecraft:nearest_living_entities",
            SensorType::NearestPlayers => "minecraft:nearest_players",
            SensorType::HurtBy => "minecraft:hurt_by",
            SensorType::NearestAdult => "minecraft:nearest_adult",
            SensorType::FoodTemptations => "minecraft:food_temptations",
        }
    }

    pub fn create(self) -> Sensor {
        Sensor::new(self)
    }
}
