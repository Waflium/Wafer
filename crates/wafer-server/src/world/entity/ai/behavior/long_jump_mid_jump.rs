use wafer_core::Vec3;

use super::behavior::{Behavior, BehaviorState};
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::data::Pose;
use crate::world::entity::sound_event;

pub const TIME_OUT_DURATION: i32 = 100;

pub struct LongJumpMidJump {
    behavior: BehaviorState,
    time_between_long_jumps: IntProvider,
    landing_sound: &'static str,
}

impl LongJumpMidJump {
    pub fn new(time_between_long_jumps: IntProvider, landing_sound: &'static str) -> LongJumpMidJump {
        LongJumpMidJump {
            behavior: BehaviorState::with_time_out(
                vec![(memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered), (memory_module_type::LONG_JUMP_MID_JUMP.erase(), MemoryStatus::ValuePresent)],
                TIME_OUT_DURATION,
            ),
            time_between_long_jumps,
            landing_sound,
        }
    }
}

impl Behavior for LongJumpMidJump {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        !level.entity(body).on_ground
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.living_mut(body).discard_friction = true;
        level.set_entity_pose(body, Pose::LongJumping);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.entity(body).on_ground {
            let e = level.entity_mut(body);
            let movement = e.delta_movement;
            e.set_delta_movement(Vec3::new(movement.x * 0.1f32 as f64, movement.y, movement.z * 0.1f32 as f64));
            level.play_sound_from_entity(None, body, sound_event(self.landing_sound), SoundSource::Neutral, 2.0, 1.0);
        }
        level.living_mut(body).discard_friction = false;
        level.set_entity_pose(body, Pose::Standing);
        let cooldown = self.time_between_long_jumps.sample(&mut level.random);
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::LONG_JUMP_MID_JUMP);
        brain.set_memory(memory_module_type::LONG_JUMP_COOLDOWN_TICKS, cooldown);
    }
}
