use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult, Record};
use crate::world::level::dimension::level_stem::{LevelStem, OVERWORLD};
use crate::world::level::levelgen::world_dimensions::WorldDimensions;

#[derive(Clone)]
pub struct WorldPreset {
    dimensions: Vec<(String, LevelStem)>,
}

impl WorldPreset {
    pub fn decode(tag: &Nbt) -> DecodeResult<WorldPreset> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let dimensions = record.required(tag, "dimensions", |dimensions| {
            codec::base_map_partial(dimensions, |key| codec::identifier(&Nbt::String(key.to_string())), LevelStem::decode)
        });
        record
            .build(|| dimensions)
            .validate(|dimensions| if dimensions.iter().any(|(key, _)| key == OVERWORLD) { Ok(()) } else { Err("Missing overworld dimension".to_string()) })
            .map(|dimensions| WorldPreset { dimensions })
    }

    pub fn validate(tag: &Nbt) -> Result<(), String> {
        match WorldPreset::decode(tag).error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn dimensions_in_order(&self) -> Vec<(String, LevelStem)> {
        WorldDimensions::keys_in_order(self.dimensions.iter().map(|(key, _)| key.as_str()))
            .into_iter()
            .filter_map(|key| self.dimensions.iter().find(|(stem_key, _)| stem_key == key).cloned())
            .collect()
    }

    pub fn create_world_dimensions(&self) -> WorldDimensions {
        WorldDimensions::new(self.dimensions_in_order(), &[]).expect("a world preset requires an overworld")
    }
}
