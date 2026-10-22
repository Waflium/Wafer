use wafer_core::{BlockPos, Direction};

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::decoration::hanging_entity::HangingView;
use crate::world::entity::decoration::item_frame::ItemFrame;
use crate::world::entity::decoration::painting::Painting;
use crate::world::entity::entity_save::apply_default_stack_config;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::block::BlockHitResult;
use crate::world::{InteractionHand, InteractionResult};

fn may_place(level: &mut ServerLevel, player: EntityId, direction: Direction, item_stack: &ItemStack, block_pos: BlockPos, item_frame_item: bool) -> bool {
    if item_frame_item {
        !level.chunks.is_outside_build_height(block_pos.y) && level.may_use_item_at(player, block_pos, direction, item_stack)
    } else {
        !direction.axis().is_vertical() && level.may_use_item_at(player, block_pos, direction, item_stack)
    }
}

fn survives(level: &mut ServerLevel, entity: &LevelEntity) -> bool {
    match entity {
        LevelEntity::Painting(painting) => {
            let view = HangingView::of(&painting.entity, painting.entity.bounding_box());
            level.hanging_view_survives(&view)
        }
        LevelEntity::ItemFrame(frame) => {
            if frame.is_fixed() {
                return true;
            }
            let view = HangingView::of(&frame.entity, frame.pop_box());
            level.item_frame_view_survives(&view)
        }
        _ => false,
    }
}

fn placement_sound(entity: &LevelEntity) -> i32 {
    match entity {
        LevelEntity::Painting(_) => crate::world::entity::sound_event("minecraft:entity.painting.place"),
        LevelEntity::ItemFrame(frame) => frame.get_place_sound(),
        _ => unreachable!("a hanging entity"),
    }
}

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_in_hand: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let clicked_face = hit.direction;
    let block_pos = pos.relative(clicked_face);
    let entity_type = item_in_hand.item.name();
    let item_frame_item = item_in_hand.item.is_instance_of("ItemFrameItem");
    if !may_place(level, player, clicked_face, &item_in_hand, block_pos, item_frame_item) {
        return InteractionResult::Fail;
    }
    let mut entity = match entity_type {
        "minecraft:painting" => match Painting::create(level, block_pos, clicked_face) {
            Some(painting) => LevelEntity::Painting(painting),
            None => return InteractionResult::CONSUME,
        },
        "minecraft:item_frame" => LevelEntity::ItemFrame(ItemFrame::new(false, block_pos, clicked_face)),
        "minecraft:glow_item_frame" => LevelEntity::ItemFrame(ItemFrame::new(true, block_pos, clicked_face)),
        _ => return InteractionResult::SUCCESS,
    };
    apply_default_stack_config(&mut entity, &item_in_hand);
    if survives(level, &entity) {
        let e = entity.entity();
        if !e.is_silent() {
            let (x, y, z) = (e.x(), e.y(), e.z());
            level.play_sound_at(None, x, y, z, placement_sound(&entity), SoundSource::Neutral, 1.0, 1.0);
        }
        let position = entity.entity().position();
        level.game_event_from_entity(Some(player), crate::world::level::gameevent::game_event::ENTITY_PLACE, position);
        level.add_fresh_entity(entity);
        item_in_hand.shrink(1);
        level.player_mut(player).update_item_in_hand(hand, &item_in_hand);
        InteractionResult::SUCCESS
    } else {
        InteractionResult::CONSUME
    }
}
