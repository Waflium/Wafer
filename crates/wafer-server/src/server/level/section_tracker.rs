use crate::world::level::lighting::dynamic_graph_min_fixed_point::{DynamicGraphMinFixedPoint, Topology};
use crate::world::level::lighting::section;

pub struct SectionTopology;

impl Topology for SectionTopology {
    const SOURCE: i64 = i64::MAX;

    fn for_each_neighbor(node: i64, mut f: impl FnMut(i64) -> bool) -> bool {
        for offset_x in -1..=1 {
            for offset_y in -1..=1 {
                for offset_z in -1..=1 {
                    if !f(section::offset(node, offset_x, offset_y, offset_z)) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

pub type SectionTracker = DynamicGraphMinFixedPoint<SectionTopology>;
