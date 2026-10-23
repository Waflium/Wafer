use wafer_core::ChunkPos;

pub use crate::world::level::lighting::dynamic_graph_min_fixed_point::TrackerLevels;
use crate::world::level::lighting::dynamic_graph_min_fixed_point::{DynamicGraphMinFixedPoint, Topology};

pub struct ChunkTopology;

impl Topology for ChunkTopology {
    const SOURCE: i64 = (1875066i64 & 0xFFFF_FFFF) | ((1875066i64 & 0xFFFF_FFFF) << 32);

    fn for_each_neighbor(node: i64, mut f: impl FnMut(i64) -> bool) -> bool {
        let pos = ChunkPos::unpack(node);
        for offset_x in -1..=1 {
            for offset_z in -1..=1 {
                if !f(ChunkPos::new(pos.x + offset_x, pos.z + offset_z).pack()) {
                    return false;
                }
            }
        }
        true
    }
}

pub type ChunkTracker = DynamicGraphMinFixedPoint<ChunkTopology>;
