use wafer_protocol::nbt::Nbt;

use crate::util::codec::{DecodeResult, Record};
use crate::world::item::component::types;
use crate::world::level::chunk::unseeded_chunk_generator::UnseededChunkGenerator;
use crate::world::level::dimension::dimension_type;

pub const OVERWORLD: &str = "minecraft:overworld";
pub const NETHER: &str = "minecraft:the_nether";
pub const END: &str = "minecraft:the_end";

#[derive(Clone)]
pub enum DimensionTypeHolder {
    Reference(String),
    Direct { decoded: Nbt, source: Nbt, json_source: bool },
}

impl DimensionTypeHolder {
    pub fn is(&self, key: &str) -> bool {
        matches!(self, DimensionTypeHolder::Reference(reference) if reference == key)
    }

    fn encode(&self) -> Nbt {
        match self {
            DimensionTypeHolder::Reference(key) => Nbt::String(key.clone()),
            DimensionTypeHolder::Direct { decoded, .. } => decoded.clone(),
        }
    }
}

#[derive(Clone)]
pub struct LevelStem {
    pub dimension_type: DimensionTypeHolder,
    pub generator: UnseededChunkGenerator,
}

impl LevelStem {
    pub fn new(dimension_type: DimensionTypeHolder, generator: UnseededChunkGenerator) -> LevelStem {
        LevelStem { dimension_type, generator }
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<LevelStem> {
        if let Err(error) = crate::util::codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let dimension_type = record.required(tag, "type", |dimension_type| {
            DecodeResult::from(types::holder("minecraft:dimension_type", dimension_type::direct_codec()).0.decode(dimension_type))
        });
        let generator = record.required(tag, "generator", UnseededChunkGenerator::decode);
        record.build(|| {
            let dimension_type = match dimension_type? {
                Nbt::String(key) => DimensionTypeHolder::Reference(key),
                direct => DimensionTypeHolder::Direct { decoded: direct, source: tag.get("type")?.clone(), json_source: crate::nbt::is_json_source() },
            };
            Some(LevelStem { dimension_type, generator: generator? })
        })
    }

    pub fn validate(tag: &Nbt) -> Result<(), String> {
        match LevelStem::decode(tag).error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn encode(&self) -> Nbt {
        Nbt::Compound(vec![("generator".into(), self.generator.encode()), ("type".into(), self.dimension_type.encode())].into())
    }
}
