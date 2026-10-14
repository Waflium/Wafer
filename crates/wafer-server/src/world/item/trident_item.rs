use wafer_core::{Vec3, mth};
use wafer_data::registry;
use wafer_protocol::nbt::Nbt;

use super::ItemStack;
use super::enchantment::enchantment_helper;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::arrow::abstract_arrow::Pickup;
use crate::world::entity::projectile::arrow::thrown_trident::ThrownTrident;
use crate::world::entity::{EntityId, MoverType, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub const THROW_THRESHOLD_TIME: i32 = 10;
pub const BASE_DAMAGE: f32 = 8.0;
pub const PROJECTILE_SHOOT_POWER: f32 = 2.5;
pub const USE_DURATION: i32 = 72000;

fn trident_sound(item_stack: &ItemStack) -> i32 {
    let sound = enchantment_helper::pick_highest_trident_sound(item_stack).and_then(|sound| match sound {
        Nbt::String(id) => crate::commands::arguments::parse_identifier(&id),
        other => other.get("sound_id").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier),
    });
    sound.and_then(|name| registry::SOUND_EVENT.iter().position(|&s| s == name)).map_or_else(|| sound_event("minecraft:item.trident.throw"), |id| id as i32)
}

pub fn release_using(level: &mut ServerLevel, item_stack: &ItemStack, entity: EntityId, remaining_time: i32) -> bool {
    if !level.is_player(entity) {
        return false;
    }
    let player = entity;
    let time_held = USE_DURATION - remaining_time;
    if time_held < THROW_THRESHOLD_TIME {
        return false;
    }
    let riptide_strength = enchantment_helper::get_trident_spin_attack_strength(level, item_stack, player);
    if riptide_strength > 0.0 && !(level.is_in_water_or_rain(player) && !level.is_passenger(player)) {
        return false;
    }
    if item_stack.next_damage_will_break() {
        return false;
    }
    let sound = trident_sound(item_stack);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    let hand = level.used_item_hand(player);
    let mut item_stack = item_stack.clone();
    item_stack.hurt_without_breaking(1, level, player);
    if riptide_strength == 0.0 {
        let thrown_item_stack = item_stack.consume_and_return(1, level, Some(player));
        level.player_mut(player).set_item_in_hand(hand, Some(item_stack));
        let projectile = ThrownTrident::from_owner(level, player, &thrown_item_stack);
        let trident = level.spawn_projectile_from_rotation(projectile, &thrown_item_stack, player, 0.0, PROJECTILE_SHOOT_POWER, 1.0);
        if level.has_infinite_materials(player) {
            level.projectile_mut(trident).abstract_arrow_mut().expect("a trident").pickup = Pickup::CreativeOnly;
        }
        level.play_sound_from_entity(None, trident, sound, SoundSource::Players, 1.0, 1.0);
        return true;
    }
    level.player_mut(player).set_item_in_hand(hand, Some(item_stack.clone()));
    let e = level.entity(player);
    let (y_rot, x_rot) = (e.y_rot, e.x_rot);
    let mut xd = -mth::sin((y_rot * mth::DEG_TO_RAD) as f64) * mth::cos((x_rot * mth::DEG_TO_RAD) as f64);
    let mut yd = -mth::sin((x_rot * mth::DEG_TO_RAD) as f64);
    let mut zd = mth::cos((y_rot * mth::DEG_TO_RAD) as f64) * mth::cos((x_rot * mth::DEG_TO_RAD) as f64);
    let dist = mth::sqrt(xd * xd + yd * yd + zd * zd);
    xd *= riptide_strength / dist;
    yd *= riptide_strength / dist;
    zd *= riptide_strength / dist;
    level.entity_mut(player).push(xd as f64, yd as f64, zd as f64);
    level.start_auto_spin_attack(player, 20, BASE_DAMAGE, item_stack);
    if level.entity(player).on_ground {
        let height_difference = 1.1999999f32;
        level.move_entity(player, MoverType::SelfMove, Vec3::new(0.0, height_difference as f64, 0.0));
    }
    level.play_sound_from_entity(None, player, sound, SoundSource::Players, 1.0, 1.0);
    true
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(item_in_hand) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    if item_in_hand.next_damage_will_break() {
        return (InteractionResult::Fail, None);
    }
    if enchantment_helper::get_trident_spin_attack_strength(level, &item_in_hand, player) > 0.0 && !level.is_in_water_or_rain(player) {
        return (InteractionResult::Fail, None);
    }
    level.start_using_item(player, hand);
    (InteractionResult::CONSUME, None)
}
