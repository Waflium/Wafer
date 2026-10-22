use super::dragon_charge_player_phase::DragonChargePlayerPhase;
use super::dragon_death_phase::DragonDeathPhase;
use super::dragon_holding_pattern_phase::DragonHoldingPatternPhase;
use super::dragon_hover_phase::DragonHoverPhase;
use super::dragon_landing_approach_phase::DragonLandingApproachPhase;
use super::dragon_landing_phase::DragonLandingPhase;
use super::dragon_sitting_attacking_phase::DragonSittingAttackingPhase;
use super::dragon_sitting_flaming_phase::DragonSittingFlamingPhase;
use super::dragon_sitting_scanning_phase::DragonSittingScanningPhase;
use super::dragon_strafe_player_phase::DragonStrafePlayerPhase;
use super::dragon_takeoff_phase::DragonTakeoffPhase;
use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::boss::enderdragon::ender_dragon::DATA_PHASE;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityId};

#[derive(Default)]
pub struct EnderDragonPhaseManager {
    pub current_phase: Option<EnderDragonPhase>,
    pub holding_pattern: DragonHoldingPatternPhase,
    pub strafe_player: DragonStrafePlayerPhase,
    pub landing_approach: DragonLandingApproachPhase,
    pub landing: DragonLandingPhase,
    pub takeoff: DragonTakeoffPhase,
    pub sitting_flaming: DragonSittingFlamingPhase,
    pub sitting_scanning: DragonSittingScanningPhase,
    pub sitting_attacking: DragonSittingAttackingPhase,
    pub charging_player: DragonChargePlayerPhase,
    pub dying: DragonDeathPhase,
    pub hovering: DragonHoverPhase,
}

impl EnderDragonPhaseManager {
    pub fn new() -> EnderDragonPhaseManager {
        EnderDragonPhaseManager { current_phase: Some(EnderDragonPhase::Hovering), ..EnderDragonPhaseManager::default() }
    }

    pub fn get_current_phase(&self) -> EnderDragonPhase {
        self.current_phase.expect("a dragon phase")
    }

    pub fn switch_phase(&mut self, entity: &mut Entity, target: EnderDragonPhase) {
        self.current_phase = Some(target);
        entity.data.set(DATA_PHASE, DataValue::Int(target.get_id()));
        self.begin(target);
    }

    fn begin(&mut self, phase: EnderDragonPhase) {
        match phase {
            EnderDragonPhase::HoldingPattern => self.holding_pattern.begin(),
            EnderDragonPhase::StrafePlayer => self.strafe_player.begin(),
            EnderDragonPhase::LandingApproach => self.landing_approach.begin(),
            EnderDragonPhase::Landing => self.landing.begin(),
            EnderDragonPhase::Takeoff => self.takeoff.begin(),
            EnderDragonPhase::SittingFlaming => self.sitting_flaming.begin(),
            EnderDragonPhase::SittingScanning => self.sitting_scanning.begin(),
            EnderDragonPhase::SittingAttacking => self.sitting_attacking.begin(),
            EnderDragonPhase::ChargingPlayer => self.charging_player.begin(),
            EnderDragonPhase::Dying => self.dying.begin(),
            EnderDragonPhase::Hovering => self.hovering.begin(),
        }
    }
}

pub fn switch_phase(mob: &mut Mob, target: EnderDragonPhase) {
    let MobKind::EnderDragon(dragon) = &mut mob.kind else { unreachable!("not an ender dragon") };
    dragon.phase_manager.switch_phase(&mut mob.entity, target);
}

impl ServerLevel {
    pub fn dragon_set_phase(&mut self, dragon: EntityId, target: EnderDragonPhase) {
        let current_phase = self.dragon_phase_manager(dragon).current_phase;
        if current_phase.is_none_or(|current| target != current) {
            if let Some(current) = current_phase {
                self.dragon_phase_end(dragon, current);
            }
            switch_phase(self.mob_mut(dragon).expect("an ender dragon"), target);
        }
    }
}
