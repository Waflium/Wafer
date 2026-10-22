use wafer_core::{Direction, Vec3, mth};
use wafer_data::{registry, registry_id};

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::decoration::armor_stand;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::BlockHitResult;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    if hit.direction == Direction::Down {
        return InteractionResult::Fail;
    }
    let place_context = BlockPlaceContext::new(level, Some(player), hand, item_stack.clone(), hit);
    let block_pos = place_context.clicked_pos();
    let rotation = place_context.rotation();
    let pos = Vec3::at_bottom_center_of(block_pos);
    let bounding_box = armor_stand::DIMENSIONS.make_bounding_box(pos);
    if !level.no_collision(&bounding_box) || !level.entity_collisions(None, &bounding_box).is_empty() || !level.get_entities(None, &bounding_box).is_empty() {
        return InteractionResult::Fail;
    }
    let Some(mut entity) = level.create_entity_type_from_stack("minecraft:armor_stand", Some(&item_stack), block_pos, EntitySpawnReason::SpawnItemUse, true, true) else {
        return InteractionResult::Fail;
    };
    let y_rot = mth::floor_f((mth::wrap_degrees(rotation - 180.0) + 22.5) / 45.0) as f32 * 45.0;
    let e = entity.entity_mut();
    let (x, y, z) = (e.x(), e.y(), e.z());
    e.snap_to(x, y, z, y_rot, 0.0);
    crate::world::level::server_level_accessor::ServerLevelAccessor::add_fresh_entity_with_passengers(level, entity);
    level.play_sound_at(None, x, y, z, registry_id(registry::SOUND_EVENT, "minecraft:entity.armor_stand.place"), SoundSource::Blocks, 0.75, 0.8);
    item_stack.shrink(1);
    level.player_mut(player).update_item_in_hand(hand, &item_stack);
    InteractionResult::SUCCESS
}
