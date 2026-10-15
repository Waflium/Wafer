use wafer_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::level::BlockGetter;

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

pub fn get_random_swimmable_pos(level: &mut ServerLevel, body: EntityId, max_horizontal_distance: i32, max_vertical_distance: i32) -> Option<Vec3> {
    let mut target_pos = default_random_pos::get_pos(level, body, max_horizontal_distance, max_vertical_distance);
    let mut count = 0;
    while let Some(pos) = target_pos {
        if level.block_state(BlockPos::containing_vec(pos)).is_pathfindable_water() {
            break;
        }
        let within_attempts = count < 10;
        count += 1;
        if !within_attempts {
            break;
        }
        target_pos = default_random_pos::get_pos(level, body, max_horizontal_distance, max_vertical_distance);
    }
    target_pos
}
