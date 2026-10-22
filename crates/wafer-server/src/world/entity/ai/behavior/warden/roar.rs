use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::behavior_utils;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;
use crate::world::entity::monster::warden::warden_ai;
use crate::world::entity::{EntityId, sound_event};

const TICKS_BEFORE_PLAYING_ROAR_SOUND: i64 = 25;
const ROAR_ANGER_INCREASE: i32 = 20;

pub struct Roar {
    state: BehaviorState,
}

impl Default for Roar {
    fn default() -> Roar {
        let entry_condition = vec![
            (memory_module_type::ROAR_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::ROAR_SOUND_COOLDOWN.erase(), MemoryStatus::Registered),
            (memory_module_type::ROAR_SOUND_DELAY.erase(), MemoryStatus::Registered),
        ];
        Roar { state: BehaviorState::with_time_out(entry_condition, warden_ai::ROAR_DURATION) }
    }
}

impl Behavior for Roar {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.set_memory_with_expiry(memory_module_type::ROAR_SOUND_DELAY, (), TICKS_BEFORE_PLAYING_ROAR_SOUND);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        let target = *level.get_brain(body).get_memory(memory_module_type::ROAR_TARGET).expect("a roar target");
        behavior_utils::look_at_entity(level, body, target);
        level.set_entity_pose(body, Pose::Roaring);
        level.warden_increase_anger_at(body, Some(target), ROAR_ANGER_INCREASE, false);
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        true
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain(body);
        if !brain.has_memory_value(memory_module_type::ROAR_SOUND_DELAY) && !brain.has_memory_value(memory_module_type::ROAR_SOUND_COOLDOWN) {
            level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::ROAR_SOUND_COOLDOWN, (), warden_ai::ROAR_DURATION as i64 - TICKS_BEFORE_PLAYING_ROAR_SOUND);
            level.entity_play_sound(body, sound_event("minecraft:entity.warden.roar"), 3.0, 1.0);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.entity(body).pose() == Pose::Roaring {
            level.set_entity_pose(body, Pose::Standing);
        }
        if let Some(&target) = level.get_brain(body).get_memory(memory_module_type::ROAR_TARGET) {
            level.warden_set_attack_target(body, target);
        }
        level.get_brain_mut(body).erase_memory(memory_module_type::ROAR_TARGET);
    }
}
