use wafer_core::random::RandomSource;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::entity::animal::equine::abstract_horse;
use crate::world::entity::entity_event;

pub struct RunAroundLikeCrazyGoal {
    speed_modifier: f64,
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
}

impl RunAroundLikeCrazyGoal {
    pub fn new(speed_modifier: f64) -> RunAroundLikeCrazyGoal {
        RunAroundLikeCrazyGoal { speed_modifier, pos_x: 0.0, pos_y: 0.0, pos_z: 0.0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_mob_controlled(mob) && !level.is_tamed(mob) && level.is_vehicle(mob) {
            let Some(pos) = default_random_pos::get_pos(level, mob, 5, 4) else { return false };
            self.pos_x = pos.x;
            self.pos_y = pos.y;
            self.pos_z = pos.z;
            return true;
        }
        false
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.pos_x, self.pos_y, self.pos_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_tamed(mob) && !level.mob(mob).expect("a horse").ai.navigation.is_done() && level.is_vehicle(mob)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.is_tamed(mob) || level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(50)) != 0 {
            return;
        }
        let Some(passenger) = level.first_passenger(mob) else { return };
        if level.is_player(passenger) {
            let temper = level.get_temper(mob);
            let max_temper = level.get_max_temper(mob);
            if max_temper > 0 && level.entity_mut(mob).random.next_int_bounded(max_temper) < temper {
                level.tame_with_name(mob, passenger);
                return;
            }
            level.modify_temper(mob, 5);
        }
        level.eject_passengers(mob);
        level.make_mad(mob);
        level.broadcast_entity_event(mob, entity_event::TAMING_FAILED);
    }
}
