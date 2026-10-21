use wafer_core::random::RandomSource;
use wafer_data::packets::play::clientbound;
use wafer_data::{EquipmentSlot, ItemUseAnimation, MobEffect, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::EntityId;
use super::data::{DataValue, ids};
use super::sound_event;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::Prediction;
use crate::world::effect::MobEffectInstance;
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component::values::{Consumable, FoodProperties, ItemStackTemplate};
use crate::world::item::component::{self as dc};
use crate::world::item::consume_effects::ConsumeEffect;
use crate::world::{InteractionHand, InteractionResult};

const FLAG_IS_USING: i8 = 1;
const FLAG_OFF_HAND: i8 = 2;
pub const FLAG_SPIN_ATTACK: i8 = 4;

const EVENT_USE_ITEM_COMPLETE: u8 = 9;

fn random_between(random: &mut impl RandomSource, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

fn triangle(random: &mut impl RandomSource, mean: f32, spread: f32) -> f32 {
    mean + spread * (random.next_float() - random.next_float())
}

fn create_effect_instance(entry: &Nbt) -> Option<MobEffectInstance> {
    let effect = MobEffect::by_name(&crate::commands::arguments::parse_identifier(entry.get("id")?.as_str()?)?)?;
    let duration = entry.get("duration").and_then(Nbt::as_i64).map_or(160, |duration| duration as i32);
    Some(MobEffectInstance::new(effect, duration, 0))
}

impl ServerLevel {
    pub fn living_entity_flags(&self, id: EntityId) -> i8 {
        match self.entity(id).data.get(ids::LIVING_ENTITY_FLAGS) {
            DataValue::Byte(b) => *b,
            _ => 0,
        }
    }

    pub fn set_living_entity_flag(&mut self, id: EntityId, flag: i8, value: bool) {
        let flags = self.living_entity_flags(id);
        let flags = if value { flags | flag } else { flags & !flag };
        self.entity_mut(id).data.set(ids::LIVING_ENTITY_FLAGS, DataValue::Byte(flags));
    }

    pub fn is_using_item(&self, id: EntityId) -> bool {
        self.living_entity_flags(id) & FLAG_IS_USING > 0
    }

    pub fn used_item_hand(&self, id: EntityId) -> InteractionHand {
        if self.living_entity_flags(id) & FLAG_OFF_HAND > 0 { InteractionHand::OffHand } else { InteractionHand::MainHand }
    }

    pub fn start_consuming(&mut self, id: EntityId, hand: InteractionHand, stack: ItemStack, consumable: &Consumable) -> (InteractionResult, Option<ItemStack>) {
        let can_consume = match stack.get(dc::FOOD).map(FoodProperties::from_nbt) {
            Some(food) if self.is_player(id) => self.can_eat(id, food.can_always_eat),
            _ => true,
        };
        if !can_consume {
            return (InteractionResult::Fail, None);
        }
        if consumable.consume_ticks() > 0 {
            self.start_using_item(id, hand);
            (InteractionResult::CONSUME, None)
        } else {
            let result = self.on_consume(id, stack, consumable);
            (InteractionResult::CONSUME, Some(result))
        }
    }

    pub fn start_using_item(&mut self, id: EntityId, hand: InteractionHand) {
        let Some(stack) = self.get_item_in_hand(id, hand).filter(|s| !s.is_empty()) else { return };
        if self.is_using_item(id) {
            return;
        }
        let living = self.living_mut(id);
        living.use_item = Some(stack.clone());
        living.use_item_remaining = crate::world::item::get_use_duration(&stack);
        let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
        let packet = super::living_entity::set_equipment(id, &[(slot, Some(stack))]);
        self.send_to_tracking_players(id, &packet);
        self.set_living_entity_flag(id, FLAG_IS_USING, true);
        self.set_living_entity_flag(id, FLAG_OFF_HAND, hand == InteractionHand::OffHand);
    }

    pub fn updating_using_item(&mut self, id: EntityId) {
        if !self.is_using_item(id) {
            return;
        }
        let hand = self.used_item_hand(id);
        let in_hand = self.get_item_in_hand(id, hand);
        let use_item = self.living(id).use_item.clone();
        if in_hand.is_some() && in_hand.as_ref().map(|s| s.item) == use_item.as_ref().map(|s| s.item) {
            self.living_mut(id).use_item = in_hand.clone();
            self.update_using_item(id, in_hand.expect("item in hand"));
        } else {
            self.stop_using_item(id);
        }
    }

    fn update_using_item(&mut self, id: EntityId, mut stack: ItemStack) {
        if self.is_player(id) {
            crate::advancements::triggers::criteria_triggers::USING_ITEM.trigger(self, id, &stack);
        }
        let remaining = self.living(id).use_item_remaining;
        if let Some(consumable) = stack.get(dc::CONSUMABLE).map(Consumable::from_nbt)
            && consumable.should_emit_particles_and_sounds(remaining)
        {
            self.emit_particles_and_sounds(id, &stack, &consumable, 5);
        }
        let before = stack.clone();
        crate::world::item::on_use_tick(self, id, &mut stack, remaining);
        if stack != before {
            let hand = self.used_item_hand(id);
            if self.is_player(id) {
                self.player_mut(id).set_item_in_hand(hand, Some(stack.clone()));
            } else {
                self.set_item_slot(id, hand.as_equipment_slot(), stack.clone());
            }
            self.living_mut(id).use_item = Some(stack.clone());
        }
        let living = self.living_mut(id);
        living.use_item_remaining -= 1;
        if living.use_item_remaining == 0 && !crate::world::item::use_on_release(&stack) {
            self.complete_using_item(id);
        }
    }

    fn complete_using_item(&mut self, id: EntityId) {
        let Some(use_item) = self.living(id).use_item.clone().filter(|s| !s.is_empty()) else { return };
        if !self.is_using_item(id) {
            return;
        }
        if let Some(player) = self.players.get(&id) {
            player.connection.send(clientbound::ENTITY_EVENT, |w| {
                w.i32(id);
                w.u8(EVENT_USE_ITEM_COMPLETE);
            });
        }
        let hand = self.used_item_hand(id);
        if Some(&use_item) != self.get_item_in_hand(id, hand).as_ref() {
            self.release_using_item(id);
            return;
        }
        let result = self.finish_using_item(id, use_item.clone());
        if result != use_item {
            self.set_item_in_hand(id, hand, result);
        }
        self.stop_using_item(id);
    }

    pub fn finish_using_item(&mut self, id: EntityId, stack: ItemStack) -> ItemStack {
        let before = stack.clone();
        let result = crate::world::item::finish_using_item(stack, self, id);
        self.apply_after_use_component_side_effects(id, result, before)
    }

    pub fn apply_after_use_component_side_effects(&mut self, id: EntityId, result: ItemStack, before: ItemStack) -> ItemStack {
        let result = match before.get(dc::USE_REMAINDER).map(ItemStackTemplate::from_nbt) {
            Some(remainder) => self.convert_into_remainder(id, remainder, result, &before),
            None => result,
        };
        if let Some(use_cooldown) = before.get(dc::USE_COOLDOWN)
            && let Some(player) = self.players.get_mut(&id)
        {
            let ticks = crate::world::item::item_cooldowns::use_cooldown_ticks(use_cooldown);
            player.cooldowns.add_cooldown(&before, ticks, &player.connection);
        }
        result
    }

    fn convert_into_remainder(&mut self, id: EntityId, remainder: ItemStackTemplate, result: ItemStack, before: &ItemStack) -> ItemStack {
        if self.has_infinite_materials(id) || result.count >= before.count {
            return result;
        }
        let remainder = remainder.create().unwrap_or_else(|| ItemStack::new(wafer_data::items::AIR, 0));
        if result.is_empty() {
            return remainder;
        }
        if !remainder.is_empty() && self.is_player(id) {
            let left = self.player_mut(id).pick_up(remainder.clone());
            if left > 0 {
                self.player_drop(id, remainder.copy_with_count(left), false, Prediction::Predicted);
            }
        }
        result
    }

    pub fn release_using_item(&mut self, id: EntityId) {
        let hand = self.used_item_hand(id);
        let in_hand = self.get_item_in_hand(id, hand);
        let use_item = self.living(id).use_item.clone();
        if use_item.as_ref().is_some_and(|s| !s.is_empty())
            && let Some(in_hand) = in_hand.filter(|in_hand| Some(in_hand.item) == use_item.as_ref().map(|s| s.item))
        {
            self.living_mut(id).use_item = Some(in_hand.clone());
            let remaining = self.living(id).use_item_remaining;
            if crate::world::item::release_using(self, &in_hand, id, remaining) {
                let current = self.get_item_in_hand(id, hand).unwrap_or_else(ItemStack::empty);
                let with_side_effects = self.apply_after_use_component_side_effects(id, current.clone(), in_hand.clone());
                if with_side_effects != current {
                    self.set_item_in_hand(id, hand, with_side_effects);
                }
            }
            if crate::world::item::use_on_release(&in_hand) {
                self.updating_using_item(id);
            }
        }
        self.stop_using_item(id);
    }

    pub fn stop_using_item(&mut self, id: EntityId) {
        self.set_living_entity_flag(id, FLAG_IS_USING, false);
        let living = self.living_mut(id);
        living.use_item = None;
        living.use_item_remaining = 0;
    }

    pub fn on_consume(&mut self, id: EntityId, mut stack: ItemStack, consumable: &Consumable) -> ItemStack {
        self.emit_particles_and_sounds(id, &stack, consumable, 16);
        if self.is_player(id) {
            self.award_stat(id, crate::stats::stats::ITEM_USED.get(stack.get_item().0), 1);
            crate::advancements::triggers::criteria_triggers::CONSUME_ITEM.trigger(self, id, &stack);
        }
        if let Some(food) = stack.get(dc::FOOD).map(FoodProperties::from_nbt) {
            self.food_properties_on_consume(id, &food, consumable);
        }
        if let Some(potion_contents) = stack.get(dc::POTION_CONTENTS) {
            let duration_scale = stack.get(dc::POTION_DURATION_SCALE).and_then(Nbt::as_f64).map_or(1.0, |scale| scale as f32);
            let effects = PotionContents::new(potion_contents).for_each_effect(duration_scale);
            self.apply_to_living_entity(id, effects);
        }
        if let Some(suspicious_stew_effects) = stack.get(dc::SUSPICIOUS_STEW_EFFECTS) {
            let effects: Vec<MobEffectInstance> = suspicious_stew_effects.as_list().iter().filter_map(create_effect_instance).collect();
            for effect in effects {
                self.add_effect(id, effect, None);
            }
        }
        if let Some(ominous_bottle_amplifier) = stack.get(dc::OMINOUS_BOTTLE_AMPLIFIER).and_then(Nbt::as_i64) {
            self.add_effect(id, MobEffectInstance::with(mob_effects::BAD_OMEN, 120000, ominous_bottle_amplifier as i32, false, false, true), None);
        }
        for effect in &consumable.on_consume_effects {
            self.apply_consume_effect(id, effect);
        }
        stack.consume(1, self, Some(id));
        stack
    }

    fn food_properties_on_consume(&mut self, id: EntityId, food: &FoodProperties, consumable: &Consumable) {
        let e = self.entity_mut(id);
        let pitch = triangle(&mut e.random, 1.0, 0.4);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.play_sound_at(None, x, y, z, consumable.sound.clone(), SoundSource::Neutral, 1.0, pitch);
        if self.is_player(id) {
            self.player_mut(id).food_data.eat(food);
            let e = self.entity_mut(id);
            let pitch = random_between(&mut e.random, 0.9, 1.0);
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.player.burp"), SoundSource::Players, 0.5, pitch);
        }
    }

    fn apply_to_living_entity(&mut self, id: EntityId, effects: Vec<MobEffectInstance>) {
        let player = self.is_player(id).then_some(id);
        for effect in effects {
            if effect.effect.info().instantaneous {
                self.apply_instantaneous_effect(effect.effect, player, player, id, effect.amplifier, 1.0);
            } else {
                self.add_effect(id, effect, None);
            }
        }
    }

    pub fn apply_consume_effect(&mut self, id: EntityId, effect: &ConsumeEffect) {
        match effect {
            ConsumeEffect::ApplyEffects { effects, probability } => {
                if self.entity_mut(id).random.next_float() >= *probability {
                    return;
                }
                for effect in effects {
                    self.add_effect(id, effect.clone(), None);
                }
            }
            ConsumeEffect::RemoveEffects { effects } => {
                for &effect in effects.contents() {
                    self.remove_effect(id, MobEffect(effect as u8));
                }
            }
            ConsumeEffect::ClearAllEffects => {
                self.remove_all_effects(id);
            }
            ConsumeEffect::PlaySound { sound } => {
                let e = self.entity(id);
                let (x, y, z) = (e.x(), e.y(), e.z());
                self.play_sound_at(None, x, y, z, sound.clone(), SoundSource::Neutral, 1.0, 1.0);
            }
            ConsumeEffect::TeleportRandomly { diameter, directional_particles } => {
                self.teleport_randomly(id, *diameter, *directional_particles);
            }
        }
    }

    fn teleport_randomly(&mut self, id: EntityId, diameter: f32, directional_particles: bool) -> bool {
        let min_y = self.chunks.min_y();
        let max_y = min_y + self.dimension_type().logical_height as i32 - 1;
        let does_not_teleport_to = wafer_data::tag("minecraft:block", "minecraft:consumable_does_not_teleport_to");
        for _ in 0..16 {
            let e = self.entity_mut(id);
            let xx = e.x() + (e.random.next_double() - 0.5) * diameter as f64;
            let yy = (e.y() + (e.random.next_double() - 0.5) * diameter as f64).clamp(min_y as f64, max_y as f64);
            let zz = e.z() + (e.random.next_double() - 0.5) * diameter as f64;
            if self.is_passenger(id) {
                self.stop_riding(id);
            }
            let old_pos = self.entity(id).position();
            if self.random_teleport(id, xx, yy, zz, true, &|state| does_not_teleport_to.contains(&(state.block().0 as i32))) {
                let (sound, source) = match self.mob(id).map(|mob| &mob.kind) {
                    Some(crate::world::entity::mob::MobKind::Fox(_)) => ("minecraft:entity.fox.teleport", SoundSource::Neutral),
                    _ => ("minecraft:item.chorus_fruit.teleport", SoundSource::Players),
                };
                let e = self.entity(id);
                let (x, y, z) = (e.x(), e.y(), e.z());
                self.play_sound_at(None, x, y, z, sound_event(sound), source, 1.0, 1.0);
                if directional_particles {
                    let origin = wafer_core::BlockPos::containing_vec(old_pos);
                    let target = self.entity(id).block_position();
                    let packed_difference = crate::util::block_util::clamped_pack_difference_in_position(origin, target, 127, 127, 127);
                    self.level_event(None, crate::server::level::events::PARTICLES_CONSUME_EFFECT_TELEPORT, origin, packed_difference);
                }
                self.reset_fall_distance(id);
                self.living_mut(id).reset_current_impulse_context();
                return true;
            }
        }
        false
    }

    fn emit_particles_and_sounds(&mut self, id: EntityId, stack: &ItemStack, consumable: &Consumable, particle_count: i32) {
        let e = self.entity_mut(id);
        let random = &mut e.random;
        let eat_volume = if random.next_boolean() { 0.5 } else { 1.0 };
        let eat_pitch = triangle(random, 1.0, 0.2);
        let drink_pitch = random_between(random, 0.9, 1.0);
        let drink = consumable.animation == ItemUseAnimation::Drink;
        let (volume, pitch) = if drink { (0.5, drink_pitch) } else { (eat_volume, eat_pitch) };
        if consumable.has_consume_particles {
            for _ in 0..particle_count {
                for _ in 0..4 {
                    random.next_float();
                }
            }
        }
        let sound: crate::sounds::sound_event::SoundEvent = if self.is_wandering_trader(id) {
            super::sound_event(crate::world::entity::npc::wanderingtrader::wandering_trader::WanderingTrader::get_consume_sound(stack)).into()
        } else {
            consumable.sound.clone()
        };
        self.entity_play_sound(id, sound, volume, pitch);
    }
}
