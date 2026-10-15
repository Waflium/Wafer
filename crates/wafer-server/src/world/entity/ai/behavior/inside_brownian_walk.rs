use wafer_core::{BlockPos, Direction};

use super::declarative::behavior_builder;
use super::long_jump_to_random_pos;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::phys::shapes::collision_context::CollisionContext;

fn loaded_and_entity_can_stand_on(level: &mut ServerLevel, pos: BlockPos, entity: EntityId) -> bool {
    if !level.has_chunk_at(pos) {
        return false;
    }
    let state = crate::world::level::BlockGetter::block_state(level, pos);
    let context = CollisionContext::of(level, entity);
    let (shape, offset) = context.get_collision_shape(state, pos);
    wafer_core::shapes::is_face_full(&shape.moved(offset[0], offset[1], offset[2]), Direction::Up)
}

pub fn create(speed_modifier: f32) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let body_pos = level.entity(body).block_position();
            if ServerLevelAccessor::can_see_sky(level, body_pos) {
                return false;
            }
            let mut poses: Vec<BlockPos> = BlockPos::between_closed(body_pos.offset(-1, -1, -1), body_pos.offset(1, 1, 1)).collect();
            long_jump_to_random_pos::shuffle_list(&mut poses);
            let target = poses.into_iter().find(|&pos| {
                !ServerLevelAccessor::can_see_sky(level, pos) && loaded_and_entity_can_stand_on(level, pos, body) && {
                    let bounding_box = level.entity(body).bounding_box();
                    level.no_collision_for(body, &bounding_box)
                }
            });
            if let Some(target) = target {
                walk_target.set(level, body, WalkTarget::of_block_pos(target, speed_modifier, 0));
            }
            true
        }
    })
}
