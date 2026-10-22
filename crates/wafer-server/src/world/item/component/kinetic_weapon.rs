use wafer_core::{Vec3, mth};
use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;

use super::codec::{as_f32, as_i32};
use super::piercing_weapon;
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::item::ItemStack;
use crate::world::level::clip::ClipBlock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Condition {
    pub max_duration_ticks: i32,
    pub min_speed: f32,
    pub min_relative_speed: f32,
}

impl Condition {
    fn from_nbt(tag: &Nbt) -> Condition {
        Condition {
            max_duration_ticks: tag.get("max_duration_ticks").map_or(0, as_i32),
            min_speed: tag.get("min_speed").map_or(0.0, as_f32),
            min_relative_speed: tag.get("min_relative_speed").map_or(0.0, as_f32),
        }
    }

    pub fn test(&self, ticks_used: i32, attacker_speed: f64, relative_speed: f64, entity_factor: f64) -> bool {
        ticks_used <= self.max_duration_ticks && attacker_speed >= self.min_speed as f64 * entity_factor && relative_speed >= self.min_relative_speed as f64 * entity_factor
    }
}

#[derive(Clone, Debug)]
pub struct KineticWeapon {
    pub contact_cooldown_ticks: i32,
    pub delay_ticks: i32,
    pub dismount_conditions: Option<Condition>,
    pub knockback_conditions: Option<Condition>,
    pub damage_conditions: Option<Condition>,
    pub damage_multiplier: f32,
    pub sound: Option<SoundEvent>,
}

impl KineticWeapon {
    pub fn from_nbt(tag: &Nbt) -> KineticWeapon {
        KineticWeapon {
            contact_cooldown_ticks: tag.get("contact_cooldown_ticks").map_or(10, as_i32),
            delay_ticks: tag.get("delay_ticks").map_or(0, as_i32),
            dismount_conditions: tag.get("dismount_conditions").map(Condition::from_nbt),
            knockback_conditions: tag.get("knockback_conditions").map(Condition::from_nbt),
            damage_conditions: tag.get("damage_conditions").map(Condition::from_nbt),
            damage_multiplier: tag.get("damage_multiplier").map_or(1.0, as_f32),
            sound: tag.get("sound").and_then(SoundEvent::from_nbt),
        }
    }

    pub fn of(item_stack: &ItemStack) -> Option<KineticWeapon> {
        item_stack.get(super::KINETIC_WEAPON).map(KineticWeapon::from_nbt)
    }

    pub fn make_sound(&self, level: &mut ServerLevel, causer: EntityId) {
        if let Some(sound) = self.sound.clone() {
            let source = level.sound_source(causer);
            let e = level.entity(causer);
            let (x, y, z) = (e.x(), e.y(), e.z());
            level.play_sound_at(Some(causer), x, y, z, sound, source, 1.0, 1.0);
        }
    }

    pub fn compute_damage_use_duration(&self) -> i32 {
        self.delay_ticks + self.damage_conditions.map_or(0, |condition| condition.max_duration_ticks)
    }

    pub fn damage_entities(&self, level: &mut ServerLevel, stack: &ItemStack, ticks_remaining: i32, living_entity: EntityId, equipment_slot: EquipmentSlot) {
        let mut ticks_used = crate::world::item::get_use_duration(stack) - ticks_remaining;
        if ticks_used < self.delay_ticks {
            return;
        }
        ticks_used -= self.delay_ticks;
        let attacker_look_vector = level.entity(living_entity).look_angle();
        let attacker_speed_projection = attacker_look_vector.dot(get_motion(level, living_entity));
        let action_factor = if level.is_player(living_entity) { 1.0f32 } else { 0.2 };
        let attack_range = level.get_attack_range_with(living_entity, stack);
        let base_mob_damage = level.living(living_entity).attributes.base_value(Attribute::AttackDamage);
        let mut affected = false;
        let hits = level
            .get_hit_entities_along(living_entity, &attack_range, |level, e| piercing_weapon::can_hit_entity(level, living_entity, e), ClipBlock::Collider)
            .right()
            .unwrap_or_default();
        for hit_result in hits {
            let other_entity = hit_result.entity;
            let was_stabbed = level.was_recently_stabbed(living_entity, other_entity, self.contact_cooldown_ticks);
            if was_stabbed {
                continue;
            }
            level.remember_stabbed_entity(living_entity, other_entity);
            let target_speed_projection = attacker_look_vector.dot(get_motion(level, other_entity));
            let relative_speed = 0.0f64.max(attacker_speed_projection - target_speed_projection);
            let factor = action_factor as f64;
            let deals_dismount = self.dismount_conditions.is_some_and(|condition| condition.test(ticks_used, attacker_speed_projection, relative_speed, factor));
            let deals_knockback = self.knockback_conditions.is_some_and(|condition| condition.test(ticks_used, attacker_speed_projection, relative_speed, factor));
            let deals_damage = self.damage_conditions.is_some_and(|condition| condition.test(ticks_used, attacker_speed_projection, relative_speed, factor));
            if deals_dismount || deals_knockback || deals_damage {
                let damage_dealt = base_mob_damage as f32 + mth::floor(relative_speed * self.damage_multiplier as f64) as f32;
                affected |= level.stab_attack(living_entity, equipment_slot, other_entity, damage_dealt, deals_damage, deals_knockback, deals_dismount);
            }
        }
        if affected {
            level.broadcast_entity_event(living_entity, 2);
            if level.is_player(living_entity) {
                let count = level.stabbed_entities(living_entity, |level, e| level.is_living(e));
                criteria_triggers::SPEAR_MOBS_TRIGGER.trigger(level, living_entity, count);
            }
        }
    }
}

pub fn get_motion(level: &ServerLevel, living_entity: EntityId) -> Vec3 {
    let entity = if !level.is_player(living_entity) && level.is_passenger(living_entity) { level.root_vehicle(living_entity) } else { living_entity };
    level.known_speed(entity).scale(20.0)
}
