use wafer_core::{AABB, Vec3};
use wafer_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::entity::boss::enderdragon::end_crystal::EndCrystal;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::gameevent::game_event;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let block_state = level.block_state(pos);
    if !block_state.is_block(blocks::OBSIDIAN) && !block_state.is_block(blocks::BEDROCK) {
        return InteractionResult::Fail;
    }
    let above = pos.above();
    if !level.block_state(above).is_air() {
        return InteractionResult::Fail;
    }
    let (x, y, z) = (above.x as f64, above.y as f64, above.z as f64);
    if !level.get_entities(None, &AABB::new(x, y, z, x + 1.0, y + 2.0, z + 1.0)).is_empty() {
        return InteractionResult::Fail;
    }
    let mut crystal = EndCrystal::of_type();
    crystal.entity.set_pos(Vec3::new(x + 0.5, y, z + 0.5));
    crystal.set_show_bottom(false);
    level.add_fresh_entity(LevelEntity::EndCrystal(crystal));
    level.game_event_at(Some(player), game_event::ENTITY_PLACE, above);
    if level.dragon_fight.is_some() {
        level.dragon_fight_try_respawn();
    }
    item_stack.shrink(1);
    level.player_mut(player).update_item_in_hand(hand, &item_stack);
    InteractionResult::SUCCESS
}
