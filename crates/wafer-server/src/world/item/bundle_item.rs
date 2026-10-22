use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use super::component::bundle_contents;
use super::component::{self as dc};
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::Prediction;
use crate::world::entity::{EntityId, sound_event};
use crate::world::inventory::abstract_container_menu::{self, AbstractContainerMenu};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

const TICKS_AFTER_FIRST_THROW: i32 = 10;
const TICKS_BETWEEN_THROWS: i32 = 2;
pub const TICKS_MAX_THROW_DURATION: i32 = 200;

pub fn override_stacked_on_other(level: &mut ServerLevel, menu: &mut AbstractContainerMenu, player: EntityId, self_stack: &mut ItemStack, slot: usize, primary: bool) -> bool {
    let Some(initial_contents) = self_stack.get(dc::BUNDLE_CONTENTS).cloned() else { return false };
    let other = level.slot_get(menu, player, slot);
    let mut contents = bundle_contents::as_mutable(&initial_contents, self_stack.bundle_selected_item());
    if primary && !other.is_empty() {
        if try_transfer(level, menu, player, &mut contents, slot) > 0 {
            play_insert_sound(level, player);
        } else {
            play_insert_fail_sound(level, player);
        }
        set_contents(self_stack, &mut contents);
        level.set_carried(player, self_stack.clone());
        broadcast_changes_on_container_menu(level, menu, player);
        true
    } else if !primary && other.is_empty() {
        if let Some(item_stack) = contents.remove_one() {
            let mut remainder = level.slot_safe_insert(menu, player, slot, item_stack.clone(), item_stack.count);
            if remainder.count > 0 {
                contents.try_insert(&mut remainder);
            } else {
                play_remove_one_sound(level, player);
            }
        }
        set_contents(self_stack, &mut contents);
        level.set_carried(player, self_stack.clone());
        broadcast_changes_on_container_menu(level, menu, player);
        true
    } else {
        false
    }
}

pub fn override_other_stacked_on_me(level: &mut ServerLevel, menu: &mut AbstractContainerMenu, player: EntityId, self_stack: &mut ItemStack, other: &mut ItemStack, slot: usize, primary: bool) -> bool {
    if primary && other.is_empty() {
        toggle_selected_item(self_stack, -1);
        level.slot_put(menu, player, slot, self_stack.clone());
        return false;
    }
    let Some(initial_contents) = self_stack.get(dc::BUNDLE_CONTENTS).cloned() else { return false };
    let mut contents = bundle_contents::as_mutable(&initial_contents, self_stack.bundle_selected_item());
    if primary && !other.is_empty() {
        if level.slot_allow_modification(menu, player, slot) && contents.try_insert(other) > 0 {
            play_insert_sound(level, player);
        } else {
            play_insert_fail_sound(level, player);
        }
        set_contents(self_stack, &mut contents);
        level.slot_put(menu, player, slot, self_stack.clone());
        level.set_carried(player, other.clone());
        broadcast_changes_on_container_menu(level, menu, player);
        true
    } else if !primary && other.is_empty() {
        if level.slot_allow_modification(menu, player, slot)
            && let Some(removed) = contents.remove_one()
        {
            play_remove_one_sound(level, player);
            *other = removed;
        }
        set_contents(self_stack, &mut contents);
        level.slot_put(menu, player, slot, self_stack.clone());
        level.set_carried(player, other.clone());
        broadcast_changes_on_container_menu(level, menu, player);
        true
    } else {
        toggle_selected_item(self_stack, -1);
        level.slot_put(menu, player, slot, self_stack.clone());
        false
    }
}

fn try_transfer(level: &mut ServerLevel, menu: &mut AbstractContainerMenu, player: EntityId, contents: &mut bundle_contents::Mutable, slot: usize) -> i32 {
    let other = level.slot_get(menu, player, slot);
    let Some(max_amount) = contents.get_max_amount_to_add_for(&other) else { return 0 };
    if bundle_contents::can_item_be_in_bundle(&other) {
        let mut taken = level.slot_safe_take(menu, player, slot, other.count, max_amount);
        contents.try_insert(&mut taken)
    } else {
        0
    }
}

fn set_contents(stack: &mut ItemStack, contents: &mut bundle_contents::Mutable) {
    let immutable = contents.to_immutable();
    stack.set_bundle_contents(immutable, contents.selected_item());
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    level.start_using_item(player, hand);
    (InteractionResult::SUCCESS, None)
}

fn drop_content_and_award(level: &mut ServerLevel, player: EntityId, item_stack: &mut ItemStack) {
    if drop_content(level, item_stack, player) {
        play_drop_contents_sound(level, player);
        level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    }
}

pub fn toggle_selected_item(stack: &mut ItemStack, selected_item: i32) {
    if let Some(initial_contents) = stack.get(dc::BUNDLE_CONTENTS).cloned() {
        let mut contents = bundle_contents::as_mutable(&initial_contents, stack.bundle_selected_item());
        contents.toggle_selected_item(selected_item);
        set_contents(stack, &mut contents);
    }
}

fn drop_content(level: &mut ServerLevel, bundle: &mut ItemStack, player: EntityId) -> bool {
    let Some(contents) = bundle.get(dc::BUNDLE_CONTENTS).cloned() else { return false };
    if bundle_contents::is_empty(&contents) {
        return false;
    }
    match remove_one_item_from_bundle(level, bundle, player, &contents) {
        Some(item_stack) => {
            level.player_drop(player, item_stack, true, Prediction::Predicted);
            true
        }
        None => false,
    }
}

fn remove_one_item_from_bundle(level: &mut ServerLevel, self_stack: &mut ItemStack, player: EntityId, initial_contents: &Nbt) -> Option<ItemStack> {
    let mut contents = bundle_contents::as_mutable(initial_contents, self_stack.bundle_selected_item());
    let removed = contents.remove_one()?;
    play_remove_one_sound(level, player);
    set_contents(self_stack, &mut contents);
    Some(removed)
}

pub fn on_use_tick(level: &mut ServerLevel, living_entity: EntityId, item_stack: &mut ItemStack, ticks_remaining: i32) {
    if level.is_player(living_entity) {
        let use_duration = TICKS_MAX_THROW_DURATION;
        let is_first_tick = ticks_remaining == use_duration;
        if is_first_tick || ticks_remaining < use_duration - TICKS_AFTER_FIRST_THROW && ticks_remaining % TICKS_BETWEEN_THROWS == 0 {
            drop_content_and_award(level, living_entity, item_stack);
        }
    }
}

pub fn on_destroyed(level: &mut ServerLevel, entity: EntityId, item: &mut ItemStack) {
    if let Some(contents) = item.get(dc::BUNDLE_CONTENTS).cloned() {
        item.set(dc::BUNDLE_CONTENTS, Some(Nbt::List(Vec::new())));
        super::item_utils::on_container_destroyed(level, entity, bundle_contents::item_copies(&contents));
    }
}

fn play_remove_one_sound(level: &mut ServerLevel, entity: EntityId) {
    let pitch = 0.8 + level.random.next_float() * 0.4;
    level.entity_play_sound(entity, sound_event("minecraft:item.bundle.remove_one"), 0.8, pitch);
}

fn play_insert_sound(level: &mut ServerLevel, entity: EntityId) {
    let pitch = 0.8 + level.random.next_float() * 0.4;
    level.entity_play_sound(entity, sound_event("minecraft:item.bundle.insert"), 0.8, pitch);
}

fn play_insert_fail_sound(level: &mut ServerLevel, entity: EntityId) {
    level.entity_play_sound(entity, sound_event("minecraft:item.bundle.insert_fail"), 1.0, 1.0);
}

fn play_drop_contents_sound(level: &mut ServerLevel, entity: EntityId) {
    let pos = level.entity(entity).block_position();
    let pitch = 0.8 + level.random.next_float() * 0.4;
    level.play_sound(None, pos, sound_event("minecraft:item.bundle.drop_contents"), SoundSource::Players, 0.8, pitch);
}

fn broadcast_changes_on_container_menu(level: &mut ServerLevel, menu: &mut AbstractContainerMenu, player: EntityId) {
    level.menu_slots_changed(menu, player, abstract_container_menu::PLAYER_INVENTORY);
}
