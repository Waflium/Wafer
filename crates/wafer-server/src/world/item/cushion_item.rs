use wafer_core::{Direction, Vec3};
use wafer_data::{Item, block_tags, items, registry, registry_id};

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::decoration::cushion;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::clip::voxel_clip;
use crate::world::{InteractionHand, InteractionResult};

const COLLISION_SHAPE_RAYCAST_EPSILON: f64 = 0.001;

pub const CUSHION: [Item; 16] = [
    items::WHITE_CUSHION,
    items::ORANGE_CUSHION,
    items::MAGENTA_CUSHION,
    items::LIGHT_BLUE_CUSHION,
    items::YELLOW_CUSHION,
    items::LIME_CUSHION,
    items::PINK_CUSHION,
    items::GRAY_CUSHION,
    items::LIGHT_GRAY_CUSHION,
    items::CYAN_CUSHION,
    items::PURPLE_CUSHION,
    items::BLUE_CUSHION,
    items::BROWN_CUSHION,
    items::GREEN_CUSHION,
    items::RED_CUSHION,
    items::BLACK_CUSHION,
];

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    let recalculated_hit = recalculate_context_for_special_collision_shapes(level, player, hit);
    if recalculated_hit.direction != Direction::Up {
        return InteractionResult::Fail;
    }
    let place_context = BlockPlaceContext::new(level, Some(player), hand, item_stack.clone(), recalculated_hit);
    let block_pos = place_context.clicked_pos();
    let entity_pos = Vec3::new(block_pos.x as f64 + 0.5, recalculated_hit.location.y, block_pos.z as f64 + 0.5);
    let spawn_aabb = entity_types::CUSHION.get_spawn_aabb(entity_pos.x, entity_pos.y, entity_pos.z);
    if !cushion::can_be_placed_at(level, &spawn_aabb) {
        return InteractionResult::Fail;
    }
    if !level.get_entities_matching(None, &spawn_aabb, |level, entity| level.is_cushion(entity)).is_empty() {
        return InteractionResult::Fail;
    }
    let Some(mut entity) = level.create_entity_type_from_stack("minecraft:cushion", Some(&item_stack), block_pos, EntitySpawnReason::SpawnItemUse, true, true) else {
        return InteractionResult::Fail;
    };
    let y_rot = Direction::from_y_rot(place_context.rotation() as f64).to_y_rot();
    entity.entity_mut().snap_to(entity_pos.x, entity_pos.y, entity_pos.z, y_rot, 0.0);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    level.cushion_destroy_if_in_fire(id);
    let e = level.entity(id);
    let (x, y, z) = (e.x(), e.y(), e.z());
    level.play_sound_at(None, x, y, z, registry_id(registry::SOUND_EVENT, "minecraft:entity.cushion.place"), SoundSource::Blocks, 0.75, 0.8);
    level.entity_game_event_by(id, crate::world::level::gameevent::game_event::ENTITY_PLACE, Some(player));
    item_stack.shrink(1);
    level.player_mut(player).update_item_in_hand(hand, &item_stack);
    InteractionResult::SUCCESS
}

fn recalculate_context_for_special_collision_shapes(level: &mut ServerLevel, player: EntityId, hit: BlockHitResult) -> BlockHitResult {
    let clicked_pos = hit.block_pos;
    let clicked_state = level.block_state(clicked_pos);
    if !clicked_state.is(&block_tags::CUSHION_USES_COLLISION_SHAPE) {
        return hit;
    }
    let ray_from = level.entity(player).eye_position();
    let ray = hit.location.subtract(ray_from);
    let ray_to = hit.location.add_vec(ray.normalize().scale(COLLISION_SHAPE_RAYCAST_EPSILON));
    voxel_clip(&clicked_state.get_collision_shape(clicked_pos), Vec3::ZERO, ray_from, ray_to, clicked_pos).unwrap_or(hit)
}
