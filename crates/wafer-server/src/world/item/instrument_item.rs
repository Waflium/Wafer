use super::ItemStack;
use super::component::{self as dc};
use super::instrument::Instrument;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let item_stack = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
    let Some(instrument) = get_instrument(&item_stack) else { return (InteractionResult::Fail, None) };
    level.start_using_item(player, hand);
    play(level, player, &instrument);
    let durability_damage = instrument.durability_damage;
    if durability_damage > 0 {
        level.hurt_and_break(player, hand.as_equipment_slot(), durability_damage);
    }
    let instrument_cooldown = instrument.use_duration;
    if instrument_cooldown > 0.0 {
        let p = level.player_mut(player);
        p.cooldowns.add_cooldown(&item_stack, (instrument_cooldown * 20.0).floor() as i32, &p.connection);
    }
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.item.0), 1);
    (InteractionResult::CONSUME, None)
}

pub fn get_use_duration(item_stack: &ItemStack) -> i32 {
    get_instrument(item_stack).map_or(0, |instrument| (instrument.use_duration * 20.0).floor() as i32)
}

fn get_instrument(item_stack: &ItemStack) -> Option<Instrument> {
    item_stack.get(dc::INSTRUMENT).and_then(Instrument::decode)
}

fn play(level: &mut ServerLevel, player: EntityId, instrument: &Instrument) {
    let volume = instrument.range / 16.0;
    level.play_sound_from_entity(Some(player), player, instrument.sound_event.clone(), SoundSource::Records, volume, 1.0);
}
