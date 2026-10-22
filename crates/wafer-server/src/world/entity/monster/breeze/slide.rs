use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};

use super::breeze_util;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::entity::data::Pose;

pub struct Slide {
    state: BehaviorState,
}

impl Default for Slide {
    fn default() -> Slide {
        let entry_condition = vec![
            (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
            (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_JUMP_COOLDOWN.erase(), MemoryStatus::ValueAbsent),
            (memory_module_type::BREEZE_SHOOT.erase(), MemoryStatus::ValueAbsent),
        ];
        Slide { state: BehaviorState::new(entry_condition) }
    }
}

fn random_point_in_middle_circle(level: &mut ServerLevel, breeze: EntityId, enemy: EntityId) -> Vec3 {
    let breeze_position = level.entity(breeze).position();
    let direction = level.entity(enemy).position().subtract(breeze_position);
    let distance = direction.length() - mth::lerp(level.entity_mut(breeze).random.next_double(), 8.0, 4.0);
    let target = direction.normalize().multiply(distance, distance, distance);
    breeze_position.add_vec(target)
}

impl Behavior for Slide {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, breeze: EntityId) -> bool {
        let e = level.entity(breeze);
        e.on_ground && !e.is_in_water() && e.pose() == Pose::Standing
    }

    fn start(&mut self, level: &mut ServerLevel, breeze: EntityId, _timestamp: i64) {
        let Some(enemy) = level.get_brain(breeze).get_memory(memory_module_type::ATTACK_TARGET).copied() else { return };
        let enemy_position = level.entity(enemy).position();
        let is_within_inner_ring = level.breeze_within_inner_circle_range(breeze, enemy_position);
        let mut position = None;
        if is_within_inner_ring
            && let Some(position0) = default_random_pos::get_pos_away(level, breeze, 5, 5, enemy_position)
            && breeze_util::has_line_of_sight(level, breeze, position0)
            && level.entity(enemy).distance_to_sqr_xyz(position0.x, position0.y, position0.z) > level.entity(enemy).distance_to_sqr(level.entity(breeze))
        {
            position = Some(position0);
        }
        let position = match position {
            Some(position) => position,
            None if level.entity_mut(breeze).random.next_boolean() => breeze_util::random_point_behind_target(level, enemy, breeze),
            None => random_point_in_middle_circle(level, breeze, enemy),
        };
        level.get_brain_mut(breeze).set_memory(memory_module_type::WALK_TARGET, WalkTarget::of_block_pos(BlockPos::containing_vec(position), 0.6, 1));
    }
}
