use wafer_core::random::RandomSource;

use super::behavior::{Behavior, BehaviorState};
use super::block_pos_tracker::BlockPosTracker;
use super::position_tracker::PositionTracker;
use super::work_at_composter;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

const CHECK_COOLDOWN: i64 = 300;
const DISTANCE: f64 = 1.73;

pub struct WorkAtPoi {
    behavior: BehaviorState,
    last_check: i64,
    work_at_composter: bool,
}

impl WorkAtPoi {
    pub fn new() -> WorkAtPoi {
        WorkAtPoi::with_workstation(false)
    }

    pub fn work_at_composter() -> WorkAtPoi {
        WorkAtPoi::with_workstation(true)
    }

    fn with_workstation(work_at_composter: bool) -> WorkAtPoi {
        WorkAtPoi {
            behavior: BehaviorState::new(vec![(memory_module_type::JOB_SITE.erase(), MemoryStatus::ValuePresent), (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered)]),
            last_check: 0,
            work_at_composter,
        }
    }

    fn use_workstation(&self, level: &mut ServerLevel, body: EntityId) {
        if self.work_at_composter {
            work_at_composter::use_workstation(level, body);
        }
    }
}

impl Behavior for WorkAtPoi {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if level.game_time - self.last_check < CHECK_COOLDOWN {
            return false;
        }
        if level.random.next_int_bounded(2) != 0 {
            return false;
        }
        self.last_check = level.game_time;
        let target = level.get_brain(body).get_memory(memory_module_type::JOB_SITE).expect("a job site");
        target.dimension == level.dimension && target.pos.closer_to_center_than(level.entity(body).position(), DISTANCE)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::LAST_WORKED_AT_POI, timestamp);
        if let Some(global_pos) = brain.get_memory(memory_module_type::JOB_SITE).cloned() {
            brain.set_memory(memory_module_type::LOOK_TARGET, PositionTracker::BlockPos(BlockPosTracker::new(global_pos.pos)));
        }
        level.play_work_sound(body);
        self.use_workstation(level, body);
        if level.should_restock(body) {
            level.restock(body);
        }
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        match level.get_brain(body).get_memory(memory_module_type::JOB_SITE) {
            None => false,
            Some(target) => target.dimension == level.dimension && target.pos.closer_to_center_than(level.entity(body).position(), DISTANCE),
        }
    }
}
