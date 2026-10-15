use wafer_data::{Item, items, registry, registry_id};

use super::ItemStack;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::{InteractionHand, InteractionResult};

fn can_interact_with(item: Item) -> (&'static str, i32) {
    if item == items::WARPED_FUNGUS_ON_A_STICK { ("minecraft:strider", 1) } else { ("minecraft:pig", 7) }
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let (can_interact_with, consume_item_damage) = can_interact_with(item_stack.get_item());
    let vehicle = level.controlled_vehicle(player);
    if level.is_passenger(player)
        && let Some(vehicle) = vehicle.filter(|&vehicle| level.is_item_steerable(vehicle) && level.entity(vehicle).entity_type == registry_id(registry::ENTITY_TYPE, can_interact_with))
        && level.boost(vehicle)
    {
        let slot = hand.as_equipment_slot();
        level.hurt_and_break(player, slot, consume_item_damage);
        let result = match level.item_by_slot(player, slot) {
            Some(remaining) => remaining,
            None => {
                let mut replacement = item_stack.transmute_copy(items::FISHING_ROD, 1);
                if replacement.is_damageable_item() {
                    replacement.set_damage_value(0);
                }
                replacement
            }
        };
        return (InteractionResult::SUCCESS_SERVER, Some(result));
    }
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    (InteractionResult::Pass, None)
}
