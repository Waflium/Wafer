use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub struct CountDownCooldownTicks {
    behavior: BehaviorState,
    cooldown_ticks: MemoryModuleType<i32>,
}

impl CountDownCooldownTicks {
    pub fn new(cooldown_ticks: MemoryModuleType<i32>) -> CountDownCooldownTicks {
        CountDownCooldownTicks { behavior: BehaviorState::new(vec![(cooldown_ticks.erase(), MemoryStatus::ValuePresent)]), cooldown_ticks }
    }

    fn get_cooldown_tick_memory(&self, level: &ServerLevel, body: EntityId) -> Option<i32> {
        level.get_brain(body).get_memory(self.cooldown_ticks).copied()
    }
}

impl Behavior for CountDownCooldownTicks {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        self.get_cooldown_tick_memory(level, body).is_some_and(|calm_down_ticks| calm_down_ticks > 0)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let calm_down_ticks = self.get_cooldown_tick_memory(level, body).expect("a cooldown");
        level.get_brain_mut(body).set_memory(self.cooldown_ticks, calm_down_ticks - 1);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(self.cooldown_ticks);
    }
}
