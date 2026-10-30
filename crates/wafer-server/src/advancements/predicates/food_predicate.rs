use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::{Doubles, Ints, doubles_codec, ints_codec};
use crate::util::codec::{self, DataResult, DecodeResult};
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
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let level = record.with_default(tag, "level", Ints::ANY, Ints::decode_partial);
            let saturation = record.with_default(tag, "saturation", Doubles::ANY, Doubles::decode_partial);
            record.build(|| Some(FoodPredicate { level, saturation }))
        })
        .into_result()
    }

    pub fn matches(&self, food: &FoodData) -> bool {
        self.level.matches(food.food_level) && self.saturation.matches(food.saturation_level as f64)
    }
}
