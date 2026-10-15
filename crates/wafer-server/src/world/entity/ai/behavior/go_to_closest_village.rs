use wafer_core::{BlockPos, Vec3};

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;

fn section_of(pos: BlockPos) -> BlockPos {
    BlockPos::new(pos.x >> 4, pos.y >> 4, pos.z >> 4)
}

pub fn create(speed_modifier: f32, close_enough_distance: i32) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let body_pos = level.entity(body).block_position();
            if level.is_village(body_pos) {
                return false;
            }
            let sections_to_village = level.chunks.poi_manager.sections_to_village(section_of(body_pos));
            let mut target_pos: Option<Vec3> = None;
            for _ in 0..5 {
                let land_pos = land_random_pos::get_pos_weighted(level, body, 15, 7, |level, p| -(level.chunks.poi_manager.sections_to_village(section_of(p)) as f64));
                if let Some(land_pos) = land_pos {
                    let land_pos_sections_to_village = level.chunks.poi_manager.sections_to_village(section_of(BlockPos::containing_vec(land_pos)));
                    if land_pos_sections_to_village < sections_to_village {
                        target_pos = Some(land_pos);
                        break;
                    }
                    if land_pos_sections_to_village == sections_to_village {
                        target_pos = Some(land_pos);
                    }
                }
            }
            if let Some(target_pos) = target_pos {
                walk_target.set(level, body, WalkTarget::of_vec(target_pos, speed_modifier, close_enough_distance));
            }
            true
        }
    })
}
