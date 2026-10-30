use wafer_protocol::nbt::Nbt;

use super::world_dimensions::WorldDimensions;
use super::world_options::WorldOptions;
use crate::util::codec::{self, DecodeResult};
use crate::util::java_throwable::JavaThrowable;

#[derive(Clone)]
pub struct WorldGenSettings {
    options: WorldOptions,
    dimensions: WorldDimensions,
}

impl WorldGenSettings {
    pub fn new(options: WorldOptions, dimensions: WorldDimensions) -> WorldGenSettings {
        WorldGenSettings { options, dimensions }
    }

    pub fn options(&self) -> &WorldOptions {
        &self.options
    }

    pub fn dimensions(&self) -> &WorldDimensions {
        &self.dimensions
    }

    pub fn decode(tag: &Nbt, caller_frames: &[&'static str]) -> Result<DecodeResult<WorldGenSettings>, JavaThrowable> {
        if let Err(error) = codec::map(tag) {
            return Ok(DecodeResult::error(error));
        }
        let options = WorldOptions::decode(tag);
        let dimensions = WorldDimensions::decode(tag, caller_frames)?;
        let error = match (dimensions.error, options.error) {
            (Some(dimensions), Some(options)) => Some(format!("{dimensions}; {options}")),
            (dimensions, options) => dimensions.or(options),
        };
        let value = options.value.zip(dimensions.value).map(|(options, dimensions)| WorldGenSettings { options, dimensions });
        Ok(DecodeResult { value, error })
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = Vec::new();
        self.options.encode(&mut fields);
        fields.push(("dimensions".into(), self.dimensions.encode()));
        Nbt::Compound(fields.into())
    }
}
