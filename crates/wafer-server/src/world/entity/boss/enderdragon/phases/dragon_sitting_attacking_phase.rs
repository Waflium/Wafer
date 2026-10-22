use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

const ROAR_DURATION: i32 = 40;

#[derive(Default)]
pub struct DragonSittingAttackingPhase {
    pub attacking_ticks: i32,
}

impl DragonSittingAttackingPhase {
    pub fn begin(&mut self) {
        self.attacking_ticks = 0;
    }
}

impl ServerLevel {
    pub fn dragon_sitting_attacking_do_server_tick(&mut self, dragon: EntityId) {
        let phase = &mut self.dragon_phase_manager_mut(dragon).sitting_attacking;
        let attacking_ticks = phase.attacking_ticks;
        phase.attacking_ticks += 1;
        if attacking_ticks >= ROAR_DURATION {
            self.dragon_set_phase(dragon, EnderDragonPhase::SittingFlaming);
        }
    }
}
