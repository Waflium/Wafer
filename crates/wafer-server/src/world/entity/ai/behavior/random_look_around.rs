use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};

use super::behavior::{Behavior, BehaviorState};
use super::block_pos_tracker::BlockPosTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

#[allow(dead_code)]
pub struct RandomLookAround {
    behavior: BehaviorState,
    interval: IntProvider,
    max_yaw: f32,
    min_pitch: f32,
    pitch_range: f32,
}

impl RandomLookAround {
    #[allow(dead_code)]
    pub fn new(interval: IntProvider, max_yaw: f32, min_pitch: f32, max_pitch: f32) -> RandomLookAround {
        assert!(min_pitch <= max_pitch, "Minimum pitch is larger than maximum pitch! {min_pitch} > {max_pitch}");
        RandomLookAround {
            behavior: BehaviorState::new(vec![
                (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::GAZE_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
            ]),
            interval,
            max_yaw,
            min_pitch,
            pitch_range: max_pitch - min_pitch,
        }
    }
}

impl Behavior for RandomLookAround {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let e = level.entity_mut(body);
        let pitch = mth::clamp_f(e.random.next_float() * self.pitch_range + self.min_pitch, -90.0, 90.0);
        let rotation = mth::wrap_degrees(e.y_rot + 2.0 * e.random.next_float() * self.max_yaw - self.max_yaw);
        let new_look_vec = Vec3::direction_from_rotation(pitch, rotation);
        let look_target = PositionTracker::BlockPos(BlockPosTracker::of_vec(e.eye_position().add_vec(new_look_vec)));
        let gaze_cooldown = self.interval.sample(&mut e.random);
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::LOOK_TARGET, look_target);
        brain.set_memory(memory_module_type::GAZE_COOLDOWN_TICKS, gaze_cooldown);
    }
}
