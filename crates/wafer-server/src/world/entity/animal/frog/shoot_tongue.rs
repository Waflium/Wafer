use super::frog;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::behavior::behavior::{Behavior, BehaviorState};
use crate::world::entity::ai::behavior::behavior_utils;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityId, RemovalReason, sound_event};

pub const TIME_OUT_DURATION: i32 = 100;
pub const CATCH_ANIMATION_DURATION: i32 = 6;
pub const TONGUE_ANIMATION_DURATION: i32 = 10;
const EATING_DISTANCE: f32 = 1.75;
const EATING_MOVEMENT_FACTOR: f32 = 0.75;
pub const UNREACHABLE_TONGUE_TARGETS_COOLDOWN_DURATION: i64 = 100;
pub const MAX_UNREACHBLE_TONGUE_TARGETS_IN_MEMORY: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    MoveToTarget,
    CatchAnimation,
    EatAnimation,
    Done,
}

pub struct ShootTongue {
    behavior: BehaviorState,
    eat_animation_timer: i32,
    calculate_path_counter: i32,
    tongue_sound: &'static str,
    eat_sound: &'static str,
    state: State,
}

impl ShootTongue {
    pub fn new(tongue_sound: &'static str, eat_sound: &'static str) -> ShootTongue {
        ShootTongue {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent),
                    (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
                ],
                TIME_OUT_DURATION,
            ),
            eat_animation_timer: 0,
            calculate_path_counter: 0,
            tongue_sound,
            eat_sound,
            state: State::Done,
        }
    }

    fn eat_entity(&self, level: &mut ServerLevel, body: EntityId) {
        level.play_sound_from_entity(None, body, sound_event(self.eat_sound), SoundSource::Neutral, 2.0, 1.0);
        if let Some(target) = level.get_tongue_target(body)
            && level.is_alive(target)
        {
            level.do_hurt_target(body, target);
            if !level.is_alive(target) {
                level.remove(target, RemovalReason::Killed);
            }
        }
    }

    fn can_pathfind_to_target(&self, level: &mut ServerLevel, body: EntityId, target: EntityId) -> bool {
        let path = level.get_navigation(body, |navigation, level| navigation.create_path_to_entity(level, body, target, 0));
        path.is_some_and(|path| path.get_dist_to_target() < EATING_DISTANCE)
    }

    fn add_unreachable_target_to_memory(&self, level: &mut ServerLevel, body: EntityId, entity: EntityId) {
        let mut unreachable_targets = level.get_brain(body).get_memory(memory_module_type::UNREACHABLE_TONGUE_TARGETS).cloned().unwrap_or_default();
        let uuid = level.entity(entity).uuid;
        let should_add_unreachable_target = !unreachable_targets.contains(&uuid);
        if unreachable_targets.len() == MAX_UNREACHBLE_TONGUE_TARGETS_IN_MEMORY && should_add_unreachable_target {
            unreachable_targets.remove(0);
        }
        if should_add_unreachable_target {
            unreachable_targets.push(uuid);
        }
        level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::UNREACHABLE_TONGUE_TARGETS, unreachable_targets, UNREACHABLE_TONGUE_TARGETS_COOLDOWN_DURATION);
    }

    fn attack_target(level: &ServerLevel, body: EntityId) -> EntityId {
        *level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).expect("an attack target")
    }
}

impl Behavior for ShootTongue {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let target = ShootTongue::attack_target(level, body);
        if level.entity_ref(target).is_none() {
            return false;
        }
        let can_pathfind_to_target = self.can_pathfind_to_target(level, body, target);
        if !can_pathfind_to_target {
            level.get_brain_mut(body).erase_memory(memory_module_type::ATTACK_TARGET);
            self.add_unreachable_target_to_memory(level, body, target);
        }
        can_pathfind_to_target && level.entity(body).pose() != Pose::Croaking && frog::can_eat(level, target)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        brain.has_memory_value(memory_module_type::ATTACK_TARGET) && self.state != State::Done && !brain.has_memory_value(memory_module_type::IS_PANICKING)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let target = ShootTongue::attack_target(level, body);
        behavior_utils::look_at_entity(level, body, target);
        frog::set_tongue_target(level.entity_mut(body), target);
        let position = level.entity(target).position();
        level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, WalkTarget::of_vec(position, 2.0, 0));
        self.calculate_path_counter = 10;
        self.state = State::MoveToTarget;
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(memory_module_type::ATTACK_TARGET);
        frog::erase_tongue_target(level.entity_mut(body));
        level.set_entity_pose(body, Pose::Standing);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let target = ShootTongue::attack_target(level, body);
        if level.entity_ref(target).is_none() {
            return;
        }
        frog::set_tongue_target(level.entity_mut(body), target);
        match self.state {
            State::MoveToTarget => {
                if level.entity(target).distance_to(level.entity(body)) < EATING_DISTANCE {
                    level.play_sound_from_entity(None, body, sound_event(self.tongue_sound), SoundSource::Neutral, 2.0, 1.0);
                    level.set_entity_pose(body, Pose::UsingTongue);
                    let (target_position, body_position) = (level.entity(target).position(), level.entity(body).position());
                    let movement = target_position.vector_to(body_position).normalize().scale(EATING_MOVEMENT_FACTOR as f64);
                    level.entity_mut(target).set_delta_movement(movement);
                    self.eat_animation_timer = 0;
                    self.state = State::CatchAnimation;
                } else if self.calculate_path_counter <= 0 {
                    let position = level.entity(target).position();
                    level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, WalkTarget::of_vec(position, 2.0, 0));
                    self.calculate_path_counter = 10;
                } else {
                    self.calculate_path_counter -= 1;
                }
            }
            State::CatchAnimation => {
                let timer = self.eat_animation_timer;
                self.eat_animation_timer += 1;
                if timer >= CATCH_ANIMATION_DURATION {
                    self.state = State::EatAnimation;
                    self.eat_entity(level, body);
                }
            }
            State::EatAnimation => {
                if self.eat_animation_timer >= TONGUE_ANIMATION_DURATION {
                    self.state = State::Done;
                } else {
                    self.eat_animation_timer += 1;
                }
            }
            State::Done => {}
        }
    }
}
