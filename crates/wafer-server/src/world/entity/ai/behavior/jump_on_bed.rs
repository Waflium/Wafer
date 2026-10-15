use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::block_tags;

use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::level::BlockGetter;

const MAX_TIME_TO_REACH_BED: i32 = 100;
const MIN_JUMPS: i32 = 3;
const MAX_JUMPS: i32 = 6;
const COOLDOWN_BETWEEN_JUMPS: i32 = 5;

pub struct JumpOnBed {
    behavior: BehaviorState,
    speed_modifier: f32,
    target_bed: Option<BlockPos>,
    remaining_time_to_reach_bed: i32,
    remaining_jumps: i32,
    remaining_cooldown_until_next_jump: i32,
}

impl JumpOnBed {
    pub fn new(speed_modifier: f32) -> JumpOnBed {
        JumpOnBed {
            behavior: BehaviorState::new(vec![(memory_module_type::NEAREST_BED.erase(), MemoryStatus::ValuePresent), (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)]),
            speed_modifier,
            target_bed: None,
            remaining_time_to_reach_bed: 0,
            remaining_jumps: 0,
            remaining_cooldown_until_next_jump: 0,
        }
    }

    fn start_walking_towards_bed(&self, level: &mut ServerLevel, body: EntityId, bed_pos: BlockPos) {
        level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, WalkTarget::of_block_pos(bed_pos, self.speed_modifier, 0));
    }

    fn near_bed(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.on_or_over_bed(level, body) || self.get_nearest_bed(level, body).is_some()
    }

    fn on_or_over_bed(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        let body_pos = level.entity(body).block_position();
        let one_below = body_pos.below();
        self.is_jumpable(level, body_pos) || self.is_jumpable(level, one_below)
    }

    fn on_bed_surface(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        let pos = level.entity(body).block_position();
        self.is_jumpable(level, pos)
    }

    fn is_jumpable(&self, level: &mut ServerLevel, body_pos: BlockPos) -> bool {
        level.block_state(body_pos).block().is(&block_tags::VILLAGER_BABIES_CAN_JUMP_ON_BED)
    }

    fn get_nearest_bed(&self, level: &ServerLevel, body: EntityId) -> Option<BlockPos> {
        level.get_brain(body).get_memory(memory_module_type::NEAREST_BED).copied()
    }

    fn tired_of_walking(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        !self.on_or_over_bed(level, body) && self.remaining_time_to_reach_bed <= 0
    }

    fn tired_of_jumping(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        self.on_or_over_bed(level, body) && self.remaining_jumps <= 0
    }
}

impl Behavior for JumpOnBed {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.is_baby(body) && self.near_bed(level, body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if let Some(target_bed) = self.get_nearest_bed(level, body) {
            self.target_bed = Some(target_bed);
            self.remaining_time_to_reach_bed = MAX_TIME_TO_REACH_BED;
            self.remaining_jumps = MIN_JUMPS + level.random.next_int_bounded(MAX_JUMPS - MIN_JUMPS + 1);
            self.remaining_cooldown_until_next_jump = 0;
            self.start_walking_towards_bed(level, body, target_bed);
        }
    }

    fn stop(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {
        self.target_bed = None;
        self.remaining_time_to_reach_bed = 0;
        self.remaining_jumps = 0;
        self.remaining_cooldown_until_next_jump = 0;
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.is_baby(body) && self.target_bed.is_some_and(|bed| self.is_jumpable(level, bed)) && !self.tired_of_walking(level, body) && !self.tired_of_jumping(level, body)
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if !self.on_or_over_bed(level, body) {
            self.remaining_time_to_reach_bed -= 1;
        } else if self.remaining_cooldown_until_next_jump > 0 {
            self.remaining_cooldown_until_next_jump -= 1;
        } else if self.on_bed_surface(level, body) {
            level.mob_mut(body).expect("a mob").ai.jump_control.jump();
            self.remaining_jumps -= 1;
            self.remaining_cooldown_until_next_jump = COOLDOWN_BETWEEN_JUMPS;
        }
    }
}
