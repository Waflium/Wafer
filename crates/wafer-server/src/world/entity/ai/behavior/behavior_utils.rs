use wafer_core::BlockPos;

use crate::server::level::ServerLevel;

pub fn find_section_closest_to_village(level: &ServerLevel, center: BlockPos, radius: i32) -> BlockPos {
    let dist_to_village = level.sections_to_village(center);
    let mut closest: Option<(BlockPos, i32)> = None;
    for z in center.z - radius..=center.z + radius {
        for y in center.y - radius..=center.y + radius {
            for x in center.x - radius..=center.x + radius {
                let section_pos = BlockPos::new(x, y, z);
                let sections_to_village = level.sections_to_village(section_pos);
                if sections_to_village < dist_to_village && closest.is_none_or(|(_, best)| sections_to_village < best) {
                    closest = Some((section_pos, sections_to_village));
                }
            }
        }
    }
    closest.map_or(center, |(section_pos, _)| section_pos)
}
