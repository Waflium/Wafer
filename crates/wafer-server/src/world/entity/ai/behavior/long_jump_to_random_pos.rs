use std::cell::RefCell;

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::blocks;

use super::behavior::{Behavior, BehaviorState};
use super::block_pos_tracker::BlockPosTracker;
use super::long_jump_util;
use super::position_tracker::PositionTracker;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::util::goal_utils;
use crate::world::level::BlockGetter;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::pathfinder::path::Path;

pub const FIND_JUMP_TRIES: i32 = 20;
const PREPARE_JUMP_DURATION: i64 = 40;
pub const MIN_PATHFIND_DISTANCE_TO_VALID_JUMP: i32 = 8;
const TIME_OUT_DURATION: i32 = 200;
const ALLOWED_ANGLES: [i32; 4] = [65, 70, 75, 80];

thread_local! {
    static SHUFFLE_RANDOM: RefCell<LegacyRandomSource> = RefCell::new(LegacyRandomSource::create());
}

#[derive(Clone, Copy, Debug)]
pub struct PossibleJump {
    pub target_pos: BlockPos,
    pub weight: i32,
}

pub struct LongJumpToRandomPos {
    behavior: BehaviorState,
    time_between_long_jumps: IntProvider,
    max_long_jump_height: i32,
    max_long_jump_width: i32,
    max_jump_velocity_multiplier: f32,
    jump_candidates: Vec<PossibleJump>,
    initial_position: Option<Vec3>,
    chosen_jump: Option<Vec3>,
    find_jump_tries: i32,
    prepare_jump_start: i64,
    get_jump_sound: fn(&ServerLevel, EntityId) -> &'static str,
    acceptable_landing_spot: fn(&mut ServerLevel, EntityId, BlockPos) -> bool,
}

pub fn default_acceptable_landing_spot(level: &mut ServerLevel, body: EntityId, target_pos: BlockPos) -> bool {
    let below = target_pos.below();
    level.block_state(below).is_solid_render() && !goal_utils::has_malus(level, body, target_pos)
}

#[cfg(test)]
pub fn set_shuffle_seed(seed: i64) {
    SHUFFLE_RANDOM.with(|random| *random.borrow_mut() = LegacyRandomSource::new(seed));
}

pub fn shuffle(list: &mut [i32]) {
    SHUFFLE_RANDOM.with(|random| {
        let mut random = random.borrow_mut();
        for i in (2..=list.len()).rev() {
            let j = random.next_int_bounded(i as i32) as usize;
            list.swap(i - 1, j);
        }
    });
}

impl LongJumpToRandomPos {
    pub fn new(
        time_between_long_jumps: IntProvider,
        max_long_jump_height: i32,
        max_long_jump_width: i32,
        max_jump_velocity_multiplier: f32,
        get_jump_sound: fn(&ServerLevel, EntityId) -> &'static str,
    ) -> LongJumpToRandomPos {
        LongJumpToRandomPos::with_landing_spot(time_between_long_jumps, max_long_jump_height, max_long_jump_width, max_jump_velocity_multiplier, get_jump_sound, default_acceptable_landing_spot)
    }

    pub fn with_landing_spot(
        time_between_long_jumps: IntProvider,
        max_long_jump_height: i32,
        max_long_jump_width: i32,
        max_jump_velocity_multiplier: f32,
        get_jump_sound: fn(&ServerLevel, EntityId) -> &'static str,
        acceptable_landing_spot: fn(&mut ServerLevel, EntityId, BlockPos) -> bool,
    ) -> LongJumpToRandomPos {
        LongJumpToRandomPos {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::LONG_JUMP_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::LONG_JUMP_MID_JUMP.erase(), MemoryStatus::ValueAbsent),
                ],
                TIME_OUT_DURATION,
            ),
            time_between_long_jumps,
            max_long_jump_height,
            max_long_jump_width,
            max_jump_velocity_multiplier,
            jump_candidates: Vec::new(),
            initial_position: None,
            chosen_jump: None,
            find_jump_tries: 0,
            prepare_jump_start: 0,
            get_jump_sound,
            acceptable_landing_spot,
        }
    }

    fn pick_candidate(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        while !self.jump_candidates.is_empty() {
            let Some(position) = self.get_jump_candidate(level) else { continue };
            let target_pos = position.target_pos;
            if self.is_acceptable_landing_position(level, body, target_pos) {
                let target_position = Vec3::at_center_of(target_pos);
                if let Some(jump_vector) = self.calculate_optimal_jump_vector(level, body, target_position) {
                    level.get_brain_mut(body).set_memory(memory_module_type::LOOK_TARGET, PositionTracker::BlockPos(BlockPosTracker::new(target_pos)));
                    let path = level.get_navigation(body, |navigation, level| navigation.create_path_with_max_path_length(level, body, target_pos, 0, MIN_PATHFIND_DISTANCE_TO_VALID_JUMP));
                    if !path.as_ref().is_some_and(Path::can_reach) {
                        self.chosen_jump = Some(jump_vector);
                        self.prepare_jump_start = timestamp;
                        return;
                    }
                }
            }
        }
    }

    fn get_jump_candidate(&mut self, level: &mut ServerLevel) -> Option<PossibleJump> {
        let total_weight: i64 = self.jump_candidates.iter().map(|jump| jump.weight as i64).sum();
        assert!(total_weight <= i32::MAX as i64, "Sum of weights must be <= 2147483647");
        if total_weight == 0 {
            return None;
        }
        let mut selection = level.random.next_int_bounded(total_weight as i32);
        let index = self.jump_candidates.iter().position(|jump| {
            selection -= jump.weight;
            selection < 0
        })?;
        Some(self.jump_candidates.remove(index))
    }

    fn is_acceptable_landing_position(&self, level: &mut ServerLevel, body: EntityId, target_pos: BlockPos) -> bool {
        let body_pos = level.entity(body).block_position();
        if body_pos.x == target_pos.x && body_pos.z == target_pos.z { false } else { (self.acceptable_landing_spot)(level, body, target_pos) }
    }

    fn calculate_optimal_jump_vector(&self, level: &ServerLevel, body: EntityId, target_pos: Vec3) -> Option<Vec3> {
        let mut allowed_angles = ALLOWED_ANGLES;
        shuffle(&mut allowed_angles);
        let max_jump_velocity = (level.attribute_value(body, Attribute::JumpStrength) * self.max_jump_velocity_multiplier as f64) as f32;
        allowed_angles.into_iter().find_map(|angle| long_jump_util::calculate_jump_vector_for_angle(level, body, target_pos, max_jump_velocity, angle, true))
    }
}

impl Behavior for LongJumpToRandomPos {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let e = level.entity(body);
        let block_pos = e.block_position();
        let can_start = e.on_ground && !e.is_in_water() && !e.is_in_lava() && !level.block_state(block_pos).is_block(blocks::HONEY_BLOCK);
        if !can_start {
            let cooldown = self.time_between_long_jumps.sample(&mut level.random) / 2;
            level.get_brain_mut(body).set_memory(memory_module_type::LONG_JUMP_COOLDOWN_TICKS, cooldown);
        }
        can_start
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let e = level.entity(body);
        let is_valid = self.initial_position.is_some_and(|initial_position| initial_position == e.position())
            && self.find_jump_tries > 0
            && !e.is_in_water()
            && (self.chosen_jump.is_some() || !self.jump_candidates.is_empty());
        if !is_valid && !level.get_brain(body).has_memory_value(memory_module_type::LONG_JUMP_MID_JUMP) {
            let cooldown = self.time_between_long_jumps.sample(&mut level.random) / 2;
            let brain = level.get_brain_mut(body);
            brain.set_memory(memory_module_type::LONG_JUMP_COOLDOWN_TICKS, cooldown);
            brain.erase_memory(memory_module_type::LOOK_TARGET);
        }
        is_valid
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        self.chosen_jump = None;
        self.find_jump_tries = FIND_JUMP_TRIES;
        let e = level.entity(body);
        self.initial_position = Some(e.position());
        let mob_pos = e.block_position();
        let (width, height) = (self.max_long_jump_width, self.max_long_jump_height);
        self.jump_candidates = BlockPos::between_closed(mob_pos.offset(-width, -height, -width), mob_pos.offset(width, height, width))
            .filter(|&pos| pos != mob_pos)
            .map(|pos| PossibleJump { target_pos: pos, weight: mth::ceil(mob_pos.dist_sqr(pos)) })
            .collect();
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        match self.chosen_jump {
            Some(chosen_jump) => {
                if timestamp - self.prepare_jump_start >= PREPARE_JUMP_DURATION {
                    let y_body_rot = level.living(body).y_body_rot;
                    level.entity_mut(body).set_y_rot(y_body_rot);
                    level.living_mut(body).discard_friction = true;
                    let org_length = chosen_jump.length();
                    let length_with_jump_boost = org_length + level.jump_boost_power(body) as f64;
                    level.entity_mut(body).set_delta_movement(chosen_jump.scale(length_with_jump_boost / org_length));
                    level.get_brain_mut(body).set_memory(memory_module_type::LONG_JUMP_MID_JUMP, true);
                    let sound = (self.get_jump_sound)(level, body);
                    level.play_sound_from_entity(None, body, sound_event(sound), SoundSource::Neutral, 1.0, 1.0);
                }
            }
            None => {
                self.find_jump_tries -= 1;
                self.pick_candidate(level, body, timestamp);
            }
        }
    }
}
