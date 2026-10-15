use wafer_core::random::RandomSource;

use crate::server::level::ServerLevel;
use crate::world::entity::animal::equine::abstract_horse;
use crate::world::entity::{EntityId, sound_event};

pub struct RandomStandGoal {
    next_stand: i32,
}

impl RandomStandGoal {
    pub fn new(ambient_stand_interval: i32) -> RandomStandGoal {
        RandomStandGoal { next_stand: -ambient_stand_interval }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.stand_if_possible(mob);
        self.play_stand_sound(level, mob);
    }

    fn play_stand_sound(&self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(ambient_stand_sound) = level.get_ambient_stand_sound(mob) {
            level.entity_play_sound(mob, sound_event(ambient_stand_sound), 1.0, 1.0);
        }
    }

    pub fn can_continue_to_use(&self) -> bool {
        false
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.next_stand += 1;
        if self.next_stand > 0 && level.entity_mut(mob).random.next_int_bounded(1000) < self.next_stand {
            self.reset_stand_interval();
            return !level.is_immobile(mob) && level.entity_mut(mob).random.next_int_bounded(10) == 0;
        }
        false
    }

    fn reset_stand_interval(&mut self) {
        self.next_stand = -abstract_horse::get_ambient_stand_interval();
    }
}
