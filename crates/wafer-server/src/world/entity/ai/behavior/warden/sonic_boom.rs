use wafer_core::mth;
use wafer_data::damage_types;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_event;
use crate::world::entity::monster::warden::warden;
use crate::world::entity::{EntityId, sound_event};

const DISTANCE_XZ: f64 = 15.0;
const DISTANCE_Y: f64 = 20.0;
const KNOCKBACK_VERTICAL: f64 = 0.5;
const KNOCKBACK_HORIZONTAL: f64 = 2.5;
pub const COOLDOWN: i32 = 40;
const TICKS_BEFORE_PLAYING_SOUND: i32 = 34;
const DURATION: i32 = 60;

pub struct SonicBoom {
    state: BehaviorState,
}

impl Default for SonicBoom {
    fn default() -> SonicBoom {
        let entry_condition = vec![
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::SONIC_BOOM_COOLDOWN.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::SONIC_BOOM_SOUND_COOLDOWN.erase(), MemoryStatus::Registered),
            (memory_module_type::SONIC_BOOM_SOUND_DELAY.erase(), MemoryStatus::Registered),
        ];
        SonicBoom { state: BehaviorState::with_time_out(entry_condition, DURATION) }
    }
}

fn closer_than(level: &ServerLevel, body: EntityId, target: EntityId) -> bool {
    level.entity(body).position().closer_than_xz_y(level.entity(target).position(), DISTANCE_XZ, DISTANCE_Y)
}

pub fn set_cooldown(level: &mut ServerLevel, body: EntityId, cooldown: i32) {
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::SONIC_BOOM_COOLDOWN, (), cooldown as i64);
}

impl Behavior for SonicBoom {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let target = *level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).expect("an attack target");
        closer_than(level, body, target)
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        true
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.set_memory_with_expiry(memory_module_type::ATTACK_COOLING_DOWN, true, DURATION as i64);
        brain.set_memory_with_expiry(memory_module_type::SONIC_BOOM_SOUND_DELAY, (), TICKS_BEFORE_PLAYING_SOUND as i64);
        level.broadcast_entity_event(body, entity_event::SONIC_CHARGE);
        level.entity_play_sound(body, sound_event("minecraft:entity.warden.sonic_charge"), 3.0, 1.0);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if let Some(&target) = level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET) {
            let position = level.entity(target).position();
            level.set_look_at(body, position.x, position.y, position.z);
        }
        let brain = level.get_brain(body);
        if brain.has_memory_value(memory_module_type::SONIC_BOOM_SOUND_DELAY) || brain.has_memory_value(memory_module_type::SONIC_BOOM_SOUND_COOLDOWN) {
            return;
        }
        level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::SONIC_BOOM_SOUND_COOLDOWN, (), (DURATION - TICKS_BEFORE_PLAYING_SOUND) as i64);
        let Some(target) = level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).copied() else { return };
        if !level.warden_can_target_entity(body, Some(target)) || !closer_than(level, body, target) {
            return;
        }
        let source = level.entity(body).position().add_vec(warden::chest_attachment(level, body));
        let delta = level.entity(target).eye_position().subtract(source);
        let normalize = delta.normalize();
        let steps = mth::floor(delta.length()) + 7;
        for i in 1..steps {
            let particle_pos = source.add_vec(normalize.scale(i as f64));
            level.send_particles("minecraft:sonic_boom", |_| {}, particle_pos.x, particle_pos.y, particle_pos.z, 1, (0.0, 0.0, 0.0), 0.0);
        }
        level.entity_play_sound(body, sound_event("minecraft:entity.warden.sonic_boom"), 3.0, 1.0);
        if level.hurt_server(target, DamageSource::entity(damage_types::SONIC_BOOM, body), 10.0) {
            let knockback_resistance = level.attribute_value(target, Attribute::KnockbackResistance);
            let knockback_vertical = KNOCKBACK_VERTICAL * (1.0 - knockback_resistance);
            let knockback_horizontal = KNOCKBACK_HORIZONTAL * (1.0 - knockback_resistance);
            level.entity_mut(target).push(normalize.x * knockback_horizontal, normalize.y * knockback_vertical, normalize.z * knockback_horizontal);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        set_cooldown(level, body, COOLDOWN);
    }
}
