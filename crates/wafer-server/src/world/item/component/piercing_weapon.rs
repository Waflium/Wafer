use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::world::InteractionHand;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::clip::ClipBlock;

#[derive(Clone, Debug)]
pub struct PiercingWeapon {
    pub deals_knockback: bool,
    pub dismounts: bool,
    pub sound: Option<SoundEvent>,
    pub hit_sound: Option<SoundEvent>,
}

impl PiercingWeapon {
    pub fn from_nbt(tag: &Nbt) -> PiercingWeapon {
        PiercingWeapon {
            deals_knockback: tag.get("deals_knockback").and_then(Nbt::as_i64).is_none_or(|v| v != 0),
            dismounts: tag.get("dismounts").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
            sound: tag.get("sound").and_then(SoundEvent::from_nbt),
            hit_sound: tag.get("hit_sound").and_then(SoundEvent::from_nbt),
        }
    }

    pub fn of(item_stack: &ItemStack) -> Option<PiercingWeapon> {
        item_stack.get(super::PIERCING_WEAPON).map(PiercingWeapon::from_nbt)
    }

    pub fn make_sound(&self, level: &mut ServerLevel, causer: EntityId) {
        if let Some(sound) = self.sound.clone() {
            play_sound(level, Some(causer), causer, sound);
        }
    }

    pub fn make_hit_sound(&self, level: &mut ServerLevel, causer: EntityId) {
        if let Some(sound) = self.hit_sound.clone() {
            play_sound(level, None, causer, sound);
        }
    }

    pub fn attack(&self, level: &mut ServerLevel, attacker: EntityId, hand: EquipmentSlot) {
        let damage = level.attribute_value(attacker, Attribute::AttackDamage) as f32;
        let weapon_item = level.item_by_slot(attacker, hand).unwrap_or_else(ItemStack::empty);
        let swing_animation = weapon_item.attack_animation();
        let attack_range = level.get_attack_range_with(attacker, &weapon_item);
        let mut hit_something = false;
        let hits = level.get_hit_entities_along(attacker, &attack_range, |level, e| can_hit_entity(level, attacker, e), ClipBlock::Collider).right().unwrap_or_default();
        for hit_result in hits {
            hit_something |= level.stab_attack(attacker, hand, hit_result.entity, damage, true, self.deals_knockback, self.dismounts);
        }
        level.on_attack(attacker);
        level.post_piercing_attack(attacker);
        if hit_something {
            self.make_hit_sound(level, attacker);
        }
        self.make_sound(level, attacker);
        level.swing_and_reset_attack_strength(attacker, InteractionHand::MainHand, swing_animation, false);
    }
}

fn play_sound(level: &mut ServerLevel, except: Option<EntityId>, causer: EntityId, sound: SoundEvent) {
    let source = level.sound_source(causer);
    let e = level.entity(causer);
    let (x, y, z) = (e.x(), e.y(), e.z());
    level.play_sound_at(except, x, y, z, sound, source, 1.0, 1.0);
}

pub fn can_hit_entity(level: &ServerLevel, jabber: EntityId, target: EntityId) -> bool {
    if level.is_invulnerable_to_piercing_weapon(target) || !level.is_alive(target) {
        false
    } else if matches!(level.find_entity(target), Some(LevelEntity::Interaction(_))) {
        true
    } else if !level.can_be_hit_by_projectile(target) {
        false
    } else if level.is_player(target) && level.is_player(jabber) && !level.can_harm_player(jabber, target) {
        false
    } else {
        !level.is_passenger_of_same_vehicle(jabber, target)
    }
}
