use wafer_core::{BlockPos, ChunkPos};
use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use super::abstract_spreading_structure_placement::AbstractSpreadingStructurePlacement;
use super::concentric_rings_structure_placement::{self, ConcentricRingsStructurePlacement};
use super::random_spread_structure_placement::{self, RandomSpreadStructurePlacement};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::Pair;
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::structure::structure_set::StructureSetHolder;

pub enum StructurePlacement {
    RandomSpread(RandomSpreadStructurePlacement),
    ConcentricRings(ConcentricRingsStructurePlacement),
    DimensionOrigin,
}

fn validate(placement: Nbt) -> Result<Nbt, String> {
    match placement.get("type").and_then(Nbt::as_str) {
        Some("minecraft:random_spread") => random_spread_structure_placement::validate(placement),
        _ => Ok(placement),
    }
}

pub fn codec() -> Pair {
    let cases = vec![
        ("minecraft:concentric_rings", concentric_rings_structure_placement::fields()),
        ("minecraft:dimension_origin", Vec::new()),
        ("minecraft:random_spread", random_spread_structure_placement::fields()),
    ];
    let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:worldgen/structure_placement"), cases }));
    (Codec::Normalize(Box::new(dispatch), validate), Stream::Dispatch)
}

impl StructurePlacement {
    pub fn parse(json: &Value) -> Result<StructurePlacement, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("placement without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "random_spread" => StructurePlacement::RandomSpread(RandomSpreadStructurePlacement::parse(json)?),
            "concentric_rings" => StructurePlacement::ConcentricRings(ConcentricRingsStructurePlacement::parse(json)?),
            "dimension_origin" => StructurePlacement::DimensionOrigin,
            other => return Err(format!("unknown structure placement {other}")),
        })
    }

    pub fn is_structure_chunk(&self, set: StructureSetHolder, state: &ChunkGeneratorStructureState, source_x: i32, source_z: i32) -> bool {
        let spreading = match self {
            StructurePlacement::RandomSpread(placement) => {
                if !placement.is_placement_chunk(state, source_x, source_z) {
                    return false;
                }
                &placement.spreading
            }
            StructurePlacement::ConcentricRings(placement) => {
                if !placement.is_placement_chunk(set, state, source_x, source_z) {
                    return false;
                }
                &placement.spreading
            }
            StructurePlacement::DimensionOrigin => {
                let origin = state.get_dimension_origin();
                return origin.x == source_x && origin.z == source_z;
            }
        };
        spreading.apply_additional_chunk_restrictions(source_x, source_z, state.get_level_seed())
            && spreading.apply_interactions_with_other_structures(state, source_x, source_z)
    }

    fn spreading(&self) -> Option<&AbstractSpreadingStructurePlacement> {
        match self {
            StructurePlacement::RandomSpread(placement) => Some(&placement.spreading),
            StructurePlacement::ConcentricRings(placement) => Some(&placement.spreading),
            StructurePlacement::DimensionOrigin => None,
        }
    }

    pub fn apply_additional_chunk_restrictions(&self, source_x: i32, source_z: i32, level_seed: i64) -> bool {
        self.spreading().is_none_or(|spreading| spreading.apply_additional_chunk_restrictions(source_x, source_z, level_seed))
    }

    pub fn get_locate_pos(&self, chunk_pos: ChunkPos) -> BlockPos {
        let offset = self.locate_offset();
        BlockPos::new(chunk_pos.min_block_x(), 0, chunk_pos.min_block_z()).offset(offset.x, offset.y, offset.z)
    }

    pub fn locate_offset(&self) -> BlockPos {
        self.spreading().map_or(BlockPos::ZERO, |spreading| spreading.locate_offset)
    }
}
