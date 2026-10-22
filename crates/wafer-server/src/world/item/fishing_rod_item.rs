use wafer_core::random::RandomSource;

use super::ItemStack;
use super::enchantment::enchantment_helper;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::fishing_hook::FishingHook;
use crate::world::entity::{EntityId, sound_event};
use crate::world::{InteractionHand, InteractionResult};

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    if let Some(fishing) = level.player(player).fishing {
        let dmg = level.fishing_hook_retrieve(fishing, &item_stack);
        level.hurt_and_break(player, hand.as_equipment_slot(), dmg);
        let e = level.entity(player);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let pitch = 0.4 / (level.random.next_float() * 0.4 + 0.8);
        level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.fishing_bobber.retrieve"), SoundSource::Neutral, 1.0, pitch);
    } else {
        let e = level.entity(player);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let pitch = 0.4 / (level.random.next_float() * 0.4 + 0.8);
        level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.fishing_bobber.throw"), SoundSource::Neutral, 0.5, pitch);
        let lure_speed = (enchantment_helper::get_fishing_time_reduction(level, &item_stack, player) * 20.0) as i32;
        let luck = enchantment_helper::get_fishing_luck_bonus(level, &item_stack, player);
        let hook = FishingHook::from_player(level, player, luck, lure_speed);
        let id = level.spawn_projectile(hook, &item_stack, |_| {});
        level.player_mut(player).fishing = Some(id);
        level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.get_item().0), 1);
    }
    (InteractionResult::SUCCESS, None)
}
