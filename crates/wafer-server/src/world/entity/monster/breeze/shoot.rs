use crate::commands::source::Anchor;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;
use crate::world::entity::projectile::hurtingprojectile::windcharge::breeze_wind_charge::BreezeWindCharge;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;

const ATTACK_RANGE_MAX_SQRT: i32 = 256;
const UNCERTAINTY_BASE: i32 = 5;
const UNCERTAINTY_MULTIPLIER: i32 = 4;
const PROJECTILE_MOVEMENT_SCALE: f32 = 0.7;
const SHOOT_INITIAL_DELAY_TICKS: i32 = 15;
const SHOOT_RECOVER_DELAY_TICKS: i32 = 4;
const SHOOT_COOLDOWN_TICKS: i32 = 10;

pub struct Shoot {
    state: BehaviorState,
}

impl Default for Shoot {
    fn default() -> Shoot {
        let entry_condition = vec![
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::BREEZE_SHOOT_COOLDOWN.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_SHOOT_CHARGING.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_SHOOT_RECOVERING.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_SHOOT.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_JUMP_TARGET.erase(), MemoryStatus::ValueAbsent),
        ];
        Shoot { state: BehaviorState::with_time_out(entry_condition, SHOOT_INITIAL_DELAY_TICKS + 1 + SHOOT_RECOVER_DELAY_TICKS) }
    }
}

fn is_target_within_range(level: &ServerLevel, body: EntityId, target: EntityId) -> bool {
    let distance_sqrt = level.entity(body).position().distance_to_sqr(level.entity(target).position());
    distance_sqrt < ATTACK_RANGE_MAX_SQRT as f64
}

impl Behavior for Shoot {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, breeze: EntityId) -> bool {
        if level.entity(breeze).pose() != Pose::Standing {
            return false;
        }
        let Some(target) = level.get_brain(breeze).get_memory(memory_module_type::ATTACK_TARGET).copied() else { return false };
        let within_range = is_target_within_range(level, breeze, target);
        if !within_range {
            level.get_brain_mut(breeze).erase_memory(memory_module_type::BREEZE_SHOOT);
        }
        within_range
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        brain.has_memory_value(memory_module_type::ATTACK_TARGET) && brain.has_memory_value(memory_module_type::BREEZE_SHOOT)
    }

    fn start(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        if level.get_brain(breeze).has_memory_value(memory_module_type::ATTACK_TARGET) {
            level.set_entity_pose(breeze, Pose::Shooting);
        }
        level.get_brain_mut(breeze).set_memory_with_expiry(memory_module_type::BREEZE_SHOOT_CHARGING, (), SHOOT_INITIAL_DELAY_TICKS as i64);
        level.entity_play_sound(breeze, sound_event("minecraft:entity.breeze.inhale"), 1.0, 1.0);
    }

    fn stop(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        if level.entity(breeze).pose() == Pose::Shooting {
            level.set_entity_pose(breeze, Pose::Standing);
        }
        let brain = level.get_brain_mut(breeze);
        brain.set_memory_with_expiry(memory_module_type::BREEZE_SHOOT_COOLDOWN, (), SHOOT_COOLDOWN_TICKS as i64);
        brain.erase_memory(memory_module_type::BREEZE_SHOOT);
    }

    fn tick(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        let Some(target) = level.get_brain(breeze).get_memory(memory_module_type::ATTACK_TARGET).copied() else { return };
        let target_position = level.entity(target).position();
        level.entity_look_at(breeze, Anchor::Eyes, target_position);
        let brain = level.get_brain(breeze);
        if brain.get_memory(memory_module_type::BREEZE_SHOOT_CHARGING).is_none() && brain.get_memory(memory_module_type::BREEZE_SHOOT_RECOVERING).is_none() {
            level.get_brain_mut(breeze).set_memory_with_expiry(memory_module_type::BREEZE_SHOOT_RECOVERING, (), SHOOT_RECOVER_DELAY_TICKS as i64);
            let t = level.entity(target);
            let y_fraction = if level.is_passenger(target) { 0.8 } else { 0.3 };
            let (target_x, target_y, target_z) = (t.x(), t.get_y(y_fraction), t.z());
            let b = level.entity(breeze);
            let xd = target_x - b.x();
            let yd = target_y - level.breeze_get_firing_y_position(breeze);
            let zd = target_z - b.z();
            let projectile = BreezeWindCharge::from_breeze(level, breeze);
            let uncertainty = (UNCERTAINTY_BASE - level.difficulty as i32 * UNCERTAINTY_MULTIPLIER) as f32;
            level.spawn_projectile_using_shoot(projectile, &ItemStack::empty(), xd, yd, zd, PROJECTILE_MOVEMENT_SCALE, uncertainty);
            level.entity_play_sound(breeze, sound_event("minecraft:entity.breeze.shoot"), 1.5, 1.0);
        }
    }
}
