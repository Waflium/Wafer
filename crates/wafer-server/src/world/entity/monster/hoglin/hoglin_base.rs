use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::damage_types;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;

pub const PROBABILITY_OF_SPAWNING_AS_BABY: f32 = 0.2;

pub fn hurt_and_throw_target(level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
    let attack_damage = level.attribute_value(body, Attribute::AttackDamage) as f32;
    let actual_damage =
        if !level.is_baby(body) && attack_damage as i32 > 0 { attack_damage / 2.0 + level.random.next_int_bounded(attack_damage as i32) as f32 } else { attack_damage };
    let damage_source = DamageSource { causing_entity: Some(body), direct_entity: Some(body), ..DamageSource::new(damage_types::MOB_ATTACK) };
    let was_hurt = level.hurt_server(target, damage_source.clone(), actual_damage);
    if was_hurt {
        level.do_post_attack_effects(target, &damage_source);
        if !level.is_baby(body) {
            throw_target(level, body, target);
        }
    }
    was_hurt
}

pub fn throw_target(level: &mut ServerLevel, body: EntityId, target: EntityId) {
    let knockback_power = level.attribute_value(body, Attribute::AttackKnockback);
    let knockback_resistance = level.attribute_value(target, Attribute::KnockbackResistance);
    let effective_knockback_power = knockback_power - knockback_resistance;
    if !(effective_knockback_power <= 0.0) {
        let (target_position, body_position) = (level.entity(target).position(), level.entity(body).position());
        let xd = target_position.x - body_position.x;
        let zd = target_position.z - body_position.z;
        let horizontal_push_angle = (level.random.next_int_bounded(21) - 10) as f32;
        let horizontal_scale = effective_knockback_power * (level.random.next_float() * 0.5 + 0.2) as f64;
        let horizontal_push_vector = Vec3::new(xd, 0.0, zd).normalize().scale(horizontal_scale).y_rot(horizontal_push_angle);
        let vertical_scale = effective_knockback_power * level.random.next_float() as f64 * 0.5;
        level.entity_push(target, horizontal_push_vector.x, vertical_scale, horizontal_push_vector.z);
        level.entity_mut(target).sync_velocity = true;
    }
}
