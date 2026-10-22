use wafer_data::{fluid_tags, items};

use super::ItemStack;
use super::alchemy::potion_contents::PotionContents;
use super::bucket_item::player_pov_hit_result;
use super::item_utils;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, LevelEntity, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::clip::ClipFluid;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let area = level.player(player).entity.bounding_box().inflate_all(2.0);
    let clouds = level.get_entities_matching(None, &area, |level, id| {
        !level.is_spectator(id)
            && matches!(level.entities.get(&id), Some(LevelEntity::AreaEffectCloud(_)))
            && level.is_alive(id)
            && level.area_effect_cloud_get_owner(id).is_some_and(|owner| level.is_ender_dragon(owner))
    });
    let item_stack = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
    if let Some(&cloud) = clouds.first() {
        if let Some(LevelEntity::AreaEffectCloud(area_effect_cloud)) = level.entities.get_mut(&cloud) {
            let radius = area_effect_cloud.get_radius();
            area_effect_cloud.set_radius(radius - 0.5);
        }
        let position = level.player(player).entity.position();
        level.play_sound_at(None, position.x, position.y, position.z, sound_event("minecraft:item.bottle.fill_dragonbreath"), SoundSource::Neutral, 1.0, 1.0);
        level.game_event_from_entity(Some(player), crate::world::level::gameevent::game_event::FLUID_PICKUP, position);
        crate::advancements::triggers::criteria_triggers::PLAYER_INTERACTED_WITH_ENTITY.trigger(level, player, &item_stack, cloud);
        let result = turn_bottle_into_item(level, player, hand, item_stack, ItemStack::new(items::DRAGON_BREATH, 1));
        return (InteractionResult::SUCCESS, Some(result));
    }
    let Some(hit_result) = player_pov_hit_result(level, player, ClipFluid::SourceOnly) else { return (InteractionResult::Pass, None) };
    let pos = hit_result.block_pos;
    if !level.may_interact(player, pos) {
        return (InteractionResult::Pass, None);
    }
    if level.fluid_state(pos).is(&fluid_tags::WATER) {
        let position = level.player(player).entity.position();
        level.play_sound_at(Some(player), position.x, position.y, position.z, sound_event("minecraft:item.bottle.fill"), SoundSource::Neutral, 1.0, 1.0);
        level.game_event_at(Some(player), crate::world::level::gameevent::game_event::FLUID_PICKUP, pos);
        let water = PotionContents::create_item_stack(items::POTION, "minecraft:water");
        let result = turn_bottle_into_item(level, player, hand, item_stack, water);
        return (InteractionResult::SUCCESS, Some(result));
    }
    (InteractionResult::Pass, None)
}

fn turn_bottle_into_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, item_stack: ItemStack, item_stack_to_turn_into: ItemStack) -> ItemStack {
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.item.0), 1);
    item_utils::create_filled_result(level, player, hand, item_stack, item_stack_to_turn_into, true)
}
