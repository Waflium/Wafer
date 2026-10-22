use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, mth};
use wafer_data::{block_tags, fluid_tags};

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::BlockGetter;

const STEPS_TO_CHECK: [i32; 6] = [0, 1, 4, 5, 6, 7];

pub struct DolphinJumpGoal {
    interval: i32,
    breached: bool,
}

impl DolphinJumpGoal {
    pub fn new(interval: i32) -> DolphinJumpGoal {
        DolphinJumpGoal { interval: reduced_tick_delay(interval), breached: false }
    }

    pub fn can_use(&self, level: &mut ServerLevel, dolphin: EntityId) -> bool {
        if level.entity_mut(dolphin).random.next_int_bounded(self.interval) != 0 {
            return false;
        }
        let motion = level.entity(dolphin).get_motion_direction();
        let (step_x, step_z) = (motion.step_x(), motion.step_z());
        let dolphin_pos = level.entity(dolphin).block_position();
        for i in STEPS_TO_CHECK {
            if !water_is_clear(level, dolphin_pos, step_x, step_z, i) || !surface_is_clear(level, dolphin_pos, step_x, step_z, i) {
                return false;
            }
        }
        true
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, dolphin: EntityId) -> bool {
        let e = level.entity(dolphin);
        let yd = e.delta_movement.y;
        (!(yd * yd < 0.03f32 as f64) || e.x_rot == 0.0 || !(e.x_rot.abs() < 10.0) || !e.is_in_water()) && !e.on_ground
    }

    pub fn start(&mut self, level: &mut ServerLevel, dolphin: EntityId) {
        let direction = level.entity(dolphin).get_motion_direction();
        let e = level.entity_mut(dolphin);
        e.set_delta_movement(e.delta_movement.add(direction.step_x() as f64 * 0.6, 0.7, direction.step_z() as f64 * 0.6));
        level.get_navigation(dolphin, |navigation, _| navigation.stop());
    }

    pub fn stop(&mut self, level: &mut ServerLevel, dolphin: EntityId) {
        level.entity_mut(dolphin).set_x_rot(0.0);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, dolphin: EntityId) {
        let already_breached = self.breached;
        if !already_breached {
            let pos = level.entity(dolphin).block_position();
            self.breached = level.fluid_state(pos).is(&fluid_tags::WATER);
        }
        if self.breached && !already_breached {
            level.entity_play_sound(dolphin, sound_event("minecraft:entity.dolphin.jump"), 1.0, 1.0);
        }
        let e = level.entity_mut(dolphin);
        let movement = e.delta_movement;
        if movement.y * movement.y < 0.03f32 as f64 && e.x_rot != 0.0 {
            let x_rot = mth::rot_lerp(0.2, e.x_rot, 0.0);
            e.set_x_rot(x_rot);
        } else if movement.length() > 1.0e-5f32 as f64 {
            let horizontal_distance = movement.horizontal_distance();
            let rotation = (-movement.y).atan2(horizontal_distance) * mth::RAD_TO_DEG as f64;
            e.set_x_rot(rotation as f32);
        }
    }
}

fn water_is_clear(level: &mut ServerLevel, dolphin_pos: BlockPos, step_x: i32, step_z: i32, current_step: i32) -> bool {
    let next_pos = dolphin_pos.offset(step_x * current_step, 0, step_z * current_step);
    level.fluid_state(next_pos).is(&fluid_tags::WATER) && !level.block_state(next_pos).is(&block_tags::BLOCKS_DOLPHIN_JUMP)
}

fn surface_is_clear(level: &mut ServerLevel, dolphin_pos: BlockPos, step_x: i32, step_z: i32, current_step: i32) -> bool {
    level.block_state(dolphin_pos.offset(step_x * current_step, 1, step_z * current_step)).is_air() && level.block_state(dolphin_pos.offset(step_x * current_step, 2, step_z * current_step)).is_air()
}
