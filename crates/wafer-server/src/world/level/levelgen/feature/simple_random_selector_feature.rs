use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::placement::placed_feature::{PlacedFeatureHolder, parse_placed_feature_set};

pub struct SimpleRandomSelectorFeature {
    features: Vec<PlacedFeatureHolder>,
}

impl SimpleRandomSelectorFeature {
    pub fn get_sub_features(&self, out: &mut Vec<u32>) {
        self.features.iter().for_each(|feature| feature.value().get_features(out));
    }

    pub fn parse(json: &Value) -> Result<SimpleRandomSelectorFeature, String> {
        Ok(SimpleRandomSelectorFeature { features: parse_placed_feature_set(json.get("features").ok_or("missing features")?)? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let index = random.next_int_bounded(self.features.len() as i32);
        self.features[index as usize].value().place(level, random, origin)
    }
}

pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    use crate::util::extra_codecs::non_empty_holder_set;
    use crate::world::item::component::types::req;
    use crate::world::level::levelgen::placement::placed_feature;
    vec![req("features", non_empty_holder_set(placed_feature::list_codec()))]
}
