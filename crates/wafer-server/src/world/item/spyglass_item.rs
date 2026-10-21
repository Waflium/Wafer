use super::ItemStack;
use super::item_utils;
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub const USE_DURATION: i32 = 1200;

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    level.entity_play_sound(player, sound_event("minecraft:item.spyglass.use"), 1.0, 1.0);
    let item = level.player(player).item_in_hand(hand).map_or(wafer_data::items::AIR, |stack| stack.item);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.0), 1);
    item_utils::start_using_instantly(level, player, hand)
}

pub fn finish_using_item(item_stack: ItemStack, level: &mut ServerLevel, entity: EntityId) -> ItemStack {
    stop_using(level, entity);
    item_stack
}

pub fn release_using(level: &mut ServerLevel, entity: EntityId) -> bool {
    stop_using(level, entity);
    true
}

fn stop_using(level: &mut ServerLevel, entity: EntityId) {
    level.entity_play_sound(entity, sound_event("minecraft:item.spyglass.stop_using"), 1.0, 1.0);
}
