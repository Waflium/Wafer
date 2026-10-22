use wafer_core::{Vec3, mth};
use wafer_data::{damage_types, mob_effects};

use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, sound_event};

pub struct ChargeAttack {
    behavior: BehaviorState,
    time_between_attacks: i32,
    charge_targeting: TargetingConditions<'static>,
    speed: f32,
    knockback_force: f32,
    max_target_detection_distance: f64,
    max_charge_distance: f64,
    charge_sound: &'static str,
    charge_velocity_vector: Vec3,
    start_position: Vec3,
}

impl ChargeAttack {
    pub fn new(
        time_between_attacks: i32,
        charge_targeting: TargetingConditions<'static>,
        speed: f32,
        knockback_force: f32,
        max_charge_distance: f64,
        max_target_detection_distance: f64,
        charge_sound: &'static str,
    ) -> ChargeAttack {
        ChargeAttack {
            behavior: BehaviorState::new(vec![
                (memory_module_type::CHARGE_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
            ]),
            time_between_attacks,
            charge_targeting,
            speed,
            knockback_force,
            max_target_detection_distance,
            max_charge_distance,
            charge_sound,
            charge_velocity_vector: Vec3::ZERO,
            start_position: Vec3::ZERO,
        }
    }

    fn deal_damage_to_target(&self, level: &mut ServerLevel, body: EntityId, target: EntityId) {
        let damage_source = DamageSource { causing_entity: Some(body), direct_entity: Some(body), ..DamageSource::new(damage_types::MOB_ATTACK) };
        let damage = level.attribute_value(body, Attribute::AttackDamage) as f32;
        if level.hurt_server(target, damage_source.clone(), damage) {
            level.do_post_attack_effects(target, &damage_source);
        }
    }

    fn deal_knock_back(&self, level: &mut ServerLevel, body: EntityId, target: EntityId) {
        let living = level.living(body);
        let movement_speed_level = living.effect(mob_effects::SPEED).map_or(0, |effect| effect.amplifier + 1);
        let movement_slowdown_level = living.effect(mob_effects::SLOWNESS).map_or(0, |effect| effect.amplifier + 1);
        let speed_boost_power = 0.25 * (movement_speed_level - movement_slowdown_level) as f32;
        let speed_factor = mth::clamp_f(self.speed * level.attribute_value(body, Attribute::MovementSpeed) as f32, 0.2, 2.0) + speed_boost_power;
        let damage_source = DamageSource { causing_entity: Some(body), direct_entity: Some(body), ..DamageSource::new(damage_types::MOB_ATTACK) };
        let damage = level.attribute_value(body, Attribute::AttackDamage) as f32;
        let old_movement = level.entity(body).delta_movement;
        level.living_entity_cause_extra_knockback(body, target, speed_factor * self.knockback_force, old_movement, &damage_source, damage, false);
    }
}

impl Behavior for ChargeAttack {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        let Some(attack_target) = brain.get_memory(memory_module_type::ATTACK_TARGET).copied() else { return false };
        if level.is_tamable_animal(body) && level.is_tame(body) {
            return false;
        }
        let position = level.entity(body).position();
        if position.subtract(self.start_position).length_sqr() >= self.max_charge_distance * self.max_charge_distance {
            return false;
        }
        if level.entity(attack_target).position().subtract(position).length_sqr() >= self.max_target_detection_distance * self.max_target_detection_distance {
            return false;
        }
        if !level.has_line_of_sight(body, attack_target) { false } else { !level.get_brain(body).has_memory_value(memory_module_type::CHARGE_COOLDOWN_TICKS) }
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.start_position = level.entity(body).position();
        let attack_candidate = *level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).expect("an attack target");
        let direction = level.entity(attack_candidate).position().subtract(self.start_position).normalize();
        self.charge_velocity_vector = direction.scale(self.speed as f64);
        if self.can_still_use(level, body, timestamp) {
            level.entity_play_sound(body, sound_event(self.charge_sound), 1.0, 1.0);
        }
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let attack_target = *level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).expect("an attack target");
        level.look_at(body, attack_target, 360.0, 360.0);
        level.entity_mut(body).set_delta_movement(self.charge_velocity_vector);
        let bounding_box = level.entity(body).bounding_box();
        let candidates = level.get_entities_matching(None, &bounding_box, |level, id| level.is_living(id));
        let closest_attack_target = candidates.into_iter().find(|&candidate| self.charge_targeting.test(level, Some(body), candidate));
        if let Some(closest_attack_target) = closest_attack_target {
            if level.has_passenger(body, closest_attack_target) {
                return;
            }
            self.deal_damage_to_target(level, body, closest_attack_target);
            self.deal_knock_back(level, body, closest_attack_target);
            self.stop(level, body, timestamp);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let time_between_attacks = self.time_between_attacks;
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::CHARGE_COOLDOWN_TICKS, time_between_attacks);
        brain.erase_memory(memory_module_type::ATTACK_TARGET);
    }
}
