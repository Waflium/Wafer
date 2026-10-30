use wafer_data::blocks;
use wafer_protocol::nbt::Nbt;

use crate::server::level::chunk_level::ChunkStatus;
use crate::world::level::chunk::Biome;
use crate::world::level::chunk::proto_chunk::ProtoChunk;

pub const UPGRADE_MIN_Y: i32 = -64;
pub const UPGRADE_HEIGHT: i32 = 64;
const RETAINED_RETROGEN_BIOMES: [&str; 3] = ["minecraft:lush_caves", "minecraft:dripstone_caves", "minecraft:deep_dark"];

#[derive(Clone, PartialEq, Debug)]
pub struct BelowZeroRetrogen {
    target_status: ChunkStatus,
    missing_bedrock: Vec<i64>,
}

impl BelowZeroRetrogen {
    pub fn decode(tag: &Nbt) -> Option<BelowZeroRetrogen> {
        let target_status = tag.get("target_status").and_then(Nbt::as_str).and_then(ChunkStatus::by_name).filter(|&status| status != ChunkStatus::Empty)?;
        let mut missing_bedrock = match tag.get("missing_bedrock") {
            Some(Nbt::LongArray(longs)) => longs.clone(),
            Some(Nbt::List(items)) if items.iter().all(|item| item.as_i64().is_some()) => items.iter().filter_map(Nbt::as_i64).collect(),
            _ => Vec::new(),
        };
        while missing_bedrock.last() == Some(&0) {
            missing_bedrock.pop();
        }
        Some(BelowZeroRetrogen { target_status, missing_bedrock })
    }

    pub fn encode(&self) -> Nbt {
        let mut tag = vec![("target_status".to_string(), Nbt::String(format!("minecraft:{}", self.target_status.get_name())))];
        if self.has_bedrock_holes() {
            tag.push(("missing_bedrock".to_string(), Nbt::LongArray(self.missing_bedrock.clone())));
        }
        Nbt::Compound(tag.into())
    }

    pub fn replace_old_bedrock(chunk: &mut ProtoChunk) {
        let deepslate = blocks::DEEPSLATE.default_state();
        for y in 0..=4 {
            for z in 0..16 {
                for x in 0..16 {
                    if chunk.block_state(x, y, z).is_block(blocks::BEDROCK) {
                        chunk.set_block_state(x, y, z, deepslate);
                    }
                }
            }
        }
    }

    pub fn apply_bedrock_mask(&self, chunk: &mut ProtoChunk) {
        let air = blocks::AIR.default_state();
        for x in 0..16 {
            for z in 0..16 {
                if self.has_bedrock_hole(x, z) {
                    for y in UPGRADE_MIN_Y..UPGRADE_MIN_Y + UPGRADE_HEIGHT {
                        chunk.set_block_state(x, y, z, air);
                    }
                }
            }
        }
    }

    pub fn target_status(&self) -> ChunkStatus {
        self.target_status
    }

    pub fn has_bedrock_holes(&self) -> bool {
        !self.missing_bedrock.is_empty()
    }

    pub fn has_bedrock_hole(&self, x: i32, z: i32) -> bool {
        let index = ((z & 15) * 16 + (x & 15)) as usize;
        self.missing_bedrock.get(index / 64).is_some_and(|word| (word >> (index % 64)) & 1 != 0)
    }

    pub fn is_retained_biome(biome: Biome) -> bool {
        RETAINED_RETROGEN_BIOMES.contains(&biome.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_matches_vanilla() {
        let tag = Nbt::Compound(vec![("target_status".into(), Nbt::String("minecraft:spawn".into())), ("missing_bedrock".into(), Nbt::LongArray(vec![5, 0]))].into());
        let retrogen = BelowZeroRetrogen::decode(&tag).unwrap();
        assert_eq!(retrogen.target_status(), ChunkStatus::Spawn);
        assert!(retrogen.has_bedrock_hole(0, 0) && !retrogen.has_bedrock_hole(1, 0) && retrogen.has_bedrock_hole(2, 0));
        assert_eq!(retrogen.encode(), Nbt::Compound(vec![("target_status".into(), Nbt::String("minecraft:spawn".into())), ("missing_bedrock".into(), Nbt::LongArray(vec![5]))].into()));
        let empty = Nbt::Compound(vec![("target_status".into(), Nbt::String("minecraft:full".into())), ("missing_bedrock".into(), Nbt::LongArray(vec![0]))].into());
        assert_eq!(BelowZeroRetrogen::decode(&empty).unwrap().encode(), Nbt::Compound(vec![("target_status".into(), Nbt::String("minecraft:full".into()))].into()));
        assert!(BelowZeroRetrogen::decode(&Nbt::Compound(vec![("target_status".into(), Nbt::String("minecraft:empty".into()))].into())).is_none());
    }
}
