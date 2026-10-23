use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::{Doubles, Ints, doubles_codec, ints_codec};
use crate::util::codec::{self, DataResult};
use crate::world::food::FoodData;
use crate::world::item::component::types::{self, Pair, def, record};

pub fn codec() -> Pair {
    record(vec![def("level", ints_codec(), types::empty_map()), def("saturation", doubles_codec(), types::empty_map())])
}

#[derive(Debug)]
pub struct FoodPredicate {
    level: Ints,
    saturation: Doubles,
}

impl FoodPredicate {
    pub const ANY: FoodPredicate = FoodPredicate { level: Ints::ANY, saturation: Doubles::ANY };

    pub fn decode(tag: &Nbt) -> DataResult<FoodPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let level = record.field(codec::decode_or(tag, "level", Ints::ANY, Ints::decode));
        let saturation = record.field(codec::decode_or(tag, "saturation", Doubles::ANY, Doubles::decode));
        record.finish()?;
        Ok(FoodPredicate { level: level.unwrap(), saturation: saturation.unwrap() })
    }

    pub fn matches(&self, food: &FoodData) -> bool {
        self.level.matches(food.food_level) && self.saturation.matches(food.saturation_level as f64)
    }
}
