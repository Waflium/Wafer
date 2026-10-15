use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};

use super::behavior::{Behavior, BehaviorState};
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::level::pathfinder::path::Path;

const MAX_COOLDOWN_BEFORE_RETRYING: i32 = 40;

pub struct MoveToTargetSink {
    behavior: BehaviorState,
    remaining_cooldown: i32,
    path: Option<Path>,
    last_target_pos: Option<BlockPos>,
    speed_modifier: f32,
}

impl Default for MoveToTargetSink {
    fn default() -> MoveToTargetSink {
        MoveToTargetSink::new(150, 250)
    }
}

impl MoveToTargetSink {
    pub fn new(min_timeout: i32, max_timeout: i32) -> MoveToTargetSink {
        MoveToTargetSink {
            behavior: BehaviorState::with_durations(
                vec![
                    (memory_module_type::CANT_REACH_WALK_TARGET_SINCE.erase(), MemoryStatus::Registered),
                    (memory_module_type::PATH.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValuePresent),
                ],
                min_timeout,
                max_timeout,
            ),
            remaining_cooldown: 0,
            path: None,
            last_target_pos: None,
            speed_modifier: 0.0,
        }
    }

    fn try_compute_path(&mut self, level: &mut ServerLevel, body: EntityId, walk_target: &WalkTarget, timestamp: i64) -> bool {
        let target_pos = walk_target.get_target().current_block_position(level);
        self.path = level.get_navigation(body, |navigation, level| navigation.create_path(level, body, target_pos, 0));
        self.speed_modifier = walk_target.get_speed_modifier();
        if reached_target(level, body, walk_target) {
            level.get_brain_mut(body).erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        } else {
            let can_reach = self.path.as_ref().is_some_and(Path::can_reach);
            let brain = level.get_brain_mut(body);
            if can_reach {
                brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
            } else if !brain.has_memory_value(memory_module_type::CANT_REACH_WALK_TARGET_SINCE) {
                brain.set_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE, timestamp);
            }
            if self.path.is_some() {
                return true;
            }
            if let Some(partial_step) = default_random_pos::get_pos_towards(level, body, 10, 7, Vec3::at_bottom_center_of(target_pos), std::f32::consts::FRAC_PI_2 as f64) {
                self.path = level.get_navigation(body, |navigation, level| navigation.create_path_to(level, body, partial_step.x, partial_step.y, partial_step.z, 0));
                return self.path.is_some();
            }
        }
        false
    }
}

fn reached_target(level: &ServerLevel, body: EntityId, walk_target: &WalkTarget) -> bool {
    walk_target.get_target().current_block_position(level).dist_manhattan(level.entity(body).block_position()) <= walk_target.get_close_enough_dist()
}

fn is_walk_target_spectator(level: &ServerLevel, walk_target: &WalkTarget) -> bool {
    match walk_target.get_target() {
        PositionTracker::Entity(entity_tracker) => level.entity_ref(entity_tracker.get_entity()).is_some() && level.is_spectator(entity_tracker.get_entity()),
        PositionTracker::BlockPos(_) => false,
    }
}

fn same_path(a: Option<&Path>, b: Option<&Path>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), b @ Some(_)) => a.same_as(b) && a.get_next_node_index() == b.expect("a path").get_next_node_index(),
        _ => false,
    }
}

impl Behavior for MoveToTargetSink {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if self.remaining_cooldown > 0 {
            self.remaining_cooldown -= 1;
            return false;
        }
        let walk_target = level.get_brain(body).get_memory(memory_module_type::WALK_TARGET).cloned().expect("a walk target");
        let reached_target = reached_target(level, body, &walk_target);
        let game_time = level.game_time;
        if !reached_target && self.try_compute_path(level, body, &walk_target, game_time) {
            self.last_target_pos = Some(walk_target.get_target().current_block_position(level));
            return true;
        }
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        if reached_target {
            brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        }
        false
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        if self.path.is_some() && self.last_target_pos.is_some() {
            let walk_target = level.get_brain(body).get_memory(memory_module_type::WALK_TARGET).cloned();
            let is_spectator = walk_target.as_ref().is_some_and(|walk_target| is_walk_target_spectator(level, walk_target));
            let navigation_done = level.mob(body).expect("a mob").ai.navigation.is_done();
            !navigation_done && walk_target.as_ref().is_some_and(|walk_target| !reached_target(level, body, walk_target)) && !is_spectator
        } else {
            false
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let walk_target = level.get_brain(body).get_memory(memory_module_type::WALK_TARGET).cloned();
        let stuck = level.mob(body).expect("a mob").ai.navigation.is_stuck();
        if walk_target.is_some_and(|walk_target| !reached_target(level, body, &walk_target)) && stuck {
            self.remaining_cooldown = level.random.next_int_bounded(MAX_COOLDOWN_BEFORE_RETRYING);
        }
        level.get_navigation(body, |navigation, _| navigation.stop());
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::PATH);
        self.path = None;
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).set_memory_optional(memory_module_type::PATH, self.path.clone());
        let (path, speed_modifier) = (self.path.clone(), self.speed_modifier as f64);
        level.get_navigation(body, |navigation, level| navigation.move_to_path(level, body, path, speed_modifier));
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let new_path = level.mob(body).expect("a mob").ai.navigation.get_path().cloned();
        if !same_path(self.path.as_ref(), new_path.as_ref()) {
            self.path = new_path.clone();
            level.get_brain_mut(body).set_memory_optional(memory_module_type::PATH, new_path.clone());
        }
        if new_path.is_some()
            && let Some(last_target_pos) = self.last_target_pos
        {
            let walk_target = level.get_brain(body).get_memory(memory_module_type::WALK_TARGET).cloned().expect("a walk target");
            let game_time = level.game_time;
            if walk_target.get_target().current_block_position(level).dist_sqr(last_target_pos) > 4.0 && self.try_compute_path(level, body, &walk_target, game_time) {
                self.last_target_pos = Some(walk_target.get_target().current_block_position(level));
                self.start(level, body, timestamp);
            }
        }
    }
}
