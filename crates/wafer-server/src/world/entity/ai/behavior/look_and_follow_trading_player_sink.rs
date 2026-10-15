use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub struct LookAndFollowTradingPlayerSink {
    behavior: BehaviorState,
    speed_modifier: f32,
}

impl LookAndFollowTradingPlayerSink {
    pub fn new(speed_modifier: f32) -> LookAndFollowTradingPlayerSink {
        LookAndFollowTradingPlayerSink {
            behavior: BehaviorState::with_time_out(
                vec![(memory_module_type::WALK_TARGET.erase(), MemoryStatus::Registered), (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered)],
                i32::MAX,
            ),
            speed_modifier,
        }
    }

    fn follow_player(&self, level: &mut ServerLevel, body: EntityId) {
        let trading_player = level.get_trading_player(body).expect("a trading player");
        let walk_target = WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, trading_player, false)), self.speed_modifier, 2);
        let look_target = PositionTracker::Entity(EntityTracker::new(level, trading_player, true));
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::WALK_TARGET, walk_target);
        brain.set_memory(memory_module_type::LOOK_TARGET, look_target);
    }
}

impl Behavior for LookAndFollowTradingPlayerSink {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let Some(trading_player) = level.get_trading_player(body) else { return false };
        level.is_alive(body) && !level.entity(body).is_in_water() && level.living(body).hurt_time <= 0 && level.entity(body).position().distance_to_sqr(level.entity(trading_player).position()) <= 16.0
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        self.check_extra_start_conditions(level, body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        self.follow_player(level, body);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::LOOK_TARGET);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        self.follow_player(level, body);
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }
}
