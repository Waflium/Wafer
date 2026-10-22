use wafer_core::Vec3;
use wafer_data::damage_types;

use crate::server::level::ServerLevel;
use crate::server::level::server_entity::set_entity_motion;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::sound_event;
use crate::world::item::ItemStack;

pub const SMASH_ATTACK_FALL_THRESHOLD: f32 = 1.5;
const SMASH_ATTACK_HEAVY_THRESHOLD: f32 = 5.0;
pub const SMASH_ATTACK_KNOCKBACK_RADIUS: f32 = 3.5;
const SMASH_ATTACK_KNOCKBACK_POWER: f32 = 0.7;

pub fn hurt_enemy(level: &mut ServerLevel, mob: EntityId, attacker: EntityId) {
    if !can_smash_attack(level, attacker) {
        return;
    }
    let e = level.entity_mut(attacker);
    let movement = e.delta_movement;
    e.set_delta_movement(Vec3::new(movement.x, 0.01f32 as f64, movement.z));
    let impact_position = calculate_impact_position(level, attacker);
    level.living_mut(attacker).set_ignore_fall_damage_from_current_impulse(true, impact_position);
    if let Some(player) = level.players.get(&attacker) {
        player.connection.send_raw(set_entity_motion(attacker, player.entity.delta_movement));
    }
    let source = level.sound_source(attacker);
    let e = level.entity(attacker);
    let (x, y, z, fall_distance) = (e.x(), e.y(), e.z(), e.fall_distance);
    if level.entity(mob).on_ground {
        if let Some(player) = level.players.get_mut(&attacker) {
            player.spawn_extra_particles_on_fall = true;
        }
        let sound = if fall_distance > SMASH_ATTACK_HEAVY_THRESHOLD as f64 { "minecraft:item.mace.smash_ground_heavy" } else { "minecraft:item.mace.smash_ground" };
        level.play_sound_at(None, x, y, z, sound_event(sound), source, 1.0, 1.0);
    } else {
        level.play_sound_at(None, x, y, z, sound_event("minecraft:item.mace.smash_air"), source, 1.0, 1.0);
    }
    knockback(level, attacker, mob);
}

fn calculate_impact_position(level: &ServerLevel, attacker: EntityId) -> Vec3 {
    let position = level.entity(attacker).position();
    match level.living(attacker).current_impulse_impact_pos {
        Some(impact_pos) if impact_pos.y <= position.y => impact_pos,
        _ => position,
    }
}

pub fn post_hurt_enemy(level: &mut ServerLevel, attacker: EntityId) {
    if can_smash_attack(level, attacker) {
        level.reset_fall_distance(attacker);
    }
}

pub fn get_attack_damage_bonus(level: &mut ServerLevel, victim: EntityId, damage_source: &DamageSource) -> f32 {
    let Some(attacker) = damage_source.direct_entity.filter(|&attacker| level.is_living(attacker)) else { return 0.0 };
    if !can_smash_attack(level, attacker) {
        return 0.0;
    }
    let fall_height_threshold1 = 3.0;
    let fall_height_threshold2 = 8.0;
    let fall_distance = level.entity(attacker).fall_distance;
    let damage = if fall_distance <= fall_height_threshold1 {
        4.0 * fall_distance
    } else if fall_distance <= fall_height_threshold2 {
        12.0 + 2.0 * (fall_distance - 3.0)
    } else {
        22.0 + fall_distance - 8.0
    };
    let weapon_item = level.get_weapon_item(attacker);
    (damage + level.modify_fall_based_damage(&weapon_item, victim, damage_source, 0.0) as f64 * fall_distance) as f32
}

fn knockback(level: &mut ServerLevel, attacker: EntityId, entity: EntityId) {
    let on_pos = level.on_pos(entity, 0.2);
    level.level_event(None, crate::server::level::events::PARTICLES_SMASH_ATTACK, on_pos, 750);
    let radius = SMASH_ATTACK_KNOCKBACK_RADIUS as f64;
    let area = level.entity(entity).bounding_box().inflate(radius, radius, radius);
    let nearby_entities = level.get_living_entities(&area, |level, nearby| knockback_predicate(level, attacker, entity, nearby));
    for nearby in nearby_entities {
        let direction = level.entity(nearby).position().subtract(level.entity(entity).position());
        let knockback_power = get_knockback_power(level, attacker, nearby, direction);
        let knockback_vector = direction.normalize().scale(knockback_power);
        if knockback_power > 0.0 {
            level.entity_mut(nearby).push(knockback_vector.x, SMASH_ATTACK_KNOCKBACK_POWER as f64, knockback_vector.z);
            if let Some(other_player) = level.players.get(&nearby) {
                other_player.connection.send_raw(set_entity_motion(nearby, other_player.entity.delta_movement));
            }
        }
    }
}

fn knockback_predicate(level: &ServerLevel, attacker: EntityId, entity: EntityId, nearby: EntityId) -> bool {
    let not_spectator = !level.is_spectator(nearby);
    let not_player = nearby != attacker && nearby != entity;
    let not_allied_to_player = !level.is_allied_to(attacker, nearby);
    let not_tamed_by_player = !(level.is_tamable_animal(nearby) && level.is_living(entity) && level.is_tame(nearby) && level.is_owned_by(nearby, entity));
    let within_range = level.entity(entity).distance_to_sqr(level.entity(nearby)) <= (SMASH_ATTACK_KNOCKBACK_RADIUS as f64).powf(2.0);
    let not_flying_in_creative = !level.players.get(&nearby).is_some_and(|player| player.game_mode.game_type.is_creative() && player.abilities_flying());
    not_spectator && not_player && not_allied_to_player && not_tamed_by_player && within_range && not_flying_in_creative
}

fn get_knockback_power(level: &ServerLevel, attacker: EntityId, nearby: EntityId, direction: Vec3) -> f64 {
    let heavy = if level.entity(attacker).fall_distance > SMASH_ATTACK_HEAVY_THRESHOLD as f64 { 2.0 } else { 1.0 };
    (SMASH_ATTACK_KNOCKBACK_RADIUS as f64 - direction.length()) * SMASH_ATTACK_KNOCKBACK_POWER as f64 * heavy * (1.0 - level.attribute_value(nearby, Attribute::KnockbackResistance))
}

pub fn can_smash_attack(level: &ServerLevel, attacker: EntityId) -> bool {
    level.entity(attacker).fall_distance > SMASH_ATTACK_FALL_THRESHOLD as f64 && !level.entity(attacker).is_fall_flying()
}

pub fn get_item_damage_source(level: &ServerLevel, attacker: EntityId) -> Option<DamageSource> {
    can_smash_attack(level, attacker).then(|| DamageSource { causing_entity: Some(attacker), direct_entity: Some(attacker), ..DamageSource::new(damage_types::MACE_SMASH) })
}

pub fn is_mace(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is_instance_of("MaceItem")
}
