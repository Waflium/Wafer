use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub fn create(speed_modifier: f32) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let pos = level.entity(body).block_position();
            if ServerLevelAccessor::can_see_sky(level, pos) {
                return false;
            }
            if let Some(land_pos) = get_outdoor_position(level, body) {
                walk_target.set(level, body, WalkTarget::of_vec(land_pos, speed_modifier, 0));
            }
            true
        }
    })
}

fn get_outdoor_position(level: &mut ServerLevel, body: EntityId) -> Option<Vec3> {
    let pos = level.entity(body).block_position();
    for _ in 0..10 {
        let random = &mut level.entity_mut(body).random;
        let dx = random.next_int_bounded(20) - 10;
        let dy = random.next_int_bounded(6) - 3;
        let dz = random.next_int_bounded(20) - 10;
        let random_pos = pos.offset(dx, dy, dz);
        if has_no_blocks_above(level, body, random_pos) {
            return Some(Vec3::at_bottom_center_of(random_pos));
        }
    }
    None
}

pub fn has_no_blocks_above(level: &mut ServerLevel, body: EntityId, target: BlockPos) -> bool {
    ServerLevelAccessor::can_see_sky(level, target) && ServerLevelAccessor::get_height(level, HeightmapType::MotionBlocking, target.x, target.z) as f64 <= level.entity(body).y()
}
