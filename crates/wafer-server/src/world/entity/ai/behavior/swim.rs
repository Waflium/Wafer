use wafer_core::random::RandomSource;
use wafer_data::{Tag, fluid_tags};

use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct Swim {
    behavior: BehaviorState,
    chance: f32,
    fluid: &'static Tag,
}

impl Swim {
    pub fn new(chance: f32) -> Swim {
        Swim::with_fluid(chance, &fluid_tags::ENTITY_FLOATABLE)
    }

    pub fn with_fluid(chance: f32, fluid: &'static Tag) -> Swim {
        Swim { behavior: BehaviorState::new(Vec::new()), chance, fluid }
    }
}

impl Behavior for Swim {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let e = level.entity(body);
        e.fluid_height(self.fluid) > e.fluid_jump_threshold() || e.is_in_lava()
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        self.check_extra_start_conditions(level, body)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let mob = level.mob_mut(body).expect("a mob");
        if mob.entity.random.next_float() < self.chance {
            mob.ai.jump_control.jump();
        }
    }
}
