use wafer_core::Vec3;

use super::abstract_dragon_phase_instance::DEFAULT_FLY_SPEED;
use super::ender_dragon_phase::EnderDragonPhase;
use super::{dragon_charge_player_phase, dragon_death_phase, dragon_hover_phase, dragon_landing_phase};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;

impl EnderDragonPhase {
    pub fn is_sitting(self) -> bool {
        matches!(self, EnderDragonPhase::SittingFlaming | EnderDragonPhase::SittingScanning | EnderDragonPhase::SittingAttacking | EnderDragonPhase::Hovering)
    }

    pub fn get_fly_speed(self) -> f32 {
        match self {
            EnderDragonPhase::ChargingPlayer => dragon_charge_player_phase::FLY_SPEED,
            EnderDragonPhase::Dying => dragon_death_phase::FLY_SPEED,
            EnderDragonPhase::Hovering => dragon_hover_phase::FLY_SPEED,
            EnderDragonPhase::Landing => dragon_landing_phase::FLY_SPEED,
            _ => DEFAULT_FLY_SPEED,
        }
    }
}

impl ServerLevel {
    pub fn dragon_phase_do_server_tick(&mut self, dragon: EntityId, phase: EnderDragonPhase) {
        match phase {
            EnderDragonPhase::HoldingPattern => self.dragon_holding_pattern_do_server_tick(dragon),
            EnderDragonPhase::StrafePlayer => self.dragon_strafe_player_do_server_tick(dragon),
            EnderDragonPhase::LandingApproach => self.dragon_landing_approach_do_server_tick(dragon),
            EnderDragonPhase::Landing => self.dragon_landing_do_server_tick(dragon),
            EnderDragonPhase::Takeoff => self.dragon_takeoff_do_server_tick(dragon),
            EnderDragonPhase::SittingFlaming => self.dragon_sitting_flaming_do_server_tick(dragon),
            EnderDragonPhase::SittingScanning => self.dragon_sitting_scanning_do_server_tick(dragon),
            EnderDragonPhase::SittingAttacking => self.dragon_sitting_attacking_do_server_tick(dragon),
            EnderDragonPhase::ChargingPlayer => self.dragon_charge_player_do_server_tick(dragon),
            EnderDragonPhase::Dying => self.dragon_death_do_server_tick(dragon),
            EnderDragonPhase::Hovering => self.dragon_hover_do_server_tick(dragon),
        }
    }

    pub fn dragon_phase_end(&mut self, dragon: EntityId, phase: EnderDragonPhase) {
        if phase == EnderDragonPhase::SittingFlaming {
            self.dragon_sitting_flaming_end(dragon);
        }
    }

    pub fn dragon_phase_get_turn_speed(&self, dragon: EntityId, phase: EnderDragonPhase) -> f32 {
        match phase {
            EnderDragonPhase::Landing => self.dragon_landing_get_turn_speed(dragon),
            _ => self.abstract_dragon_phase_get_turn_speed(dragon),
        }
    }

    pub fn dragon_phase_get_fly_target_location(&self, dragon: EntityId, phase: EnderDragonPhase) -> Option<Vec3> {
        let phases = self.dragon_phase_manager(dragon);
        match phase {
            EnderDragonPhase::HoldingPattern => phases.holding_pattern.target_location,
            EnderDragonPhase::StrafePlayer => phases.strafe_player.target_location,
            EnderDragonPhase::LandingApproach => phases.landing_approach.target_location,
            EnderDragonPhase::Landing => phases.landing.target_location,
            EnderDragonPhase::Takeoff => phases.takeoff.target_location,
            EnderDragonPhase::ChargingPlayer => phases.charging_player.target_location,
            EnderDragonPhase::Dying => phases.dying.target_location,
            EnderDragonPhase::Hovering => phases.hovering.target_location,
            EnderDragonPhase::SittingFlaming | EnderDragonPhase::SittingScanning | EnderDragonPhase::SittingAttacking => None,
        }
    }

    pub fn dragon_phase_on_hurt(&mut self, phase: EnderDragonPhase, source: &DamageSource, damage: f32) -> f32 {
        match phase {
            EnderDragonPhase::SittingFlaming | EnderDragonPhase::SittingScanning | EnderDragonPhase::SittingAttacking => self.abstract_dragon_sitting_phase_on_hurt(source, damage),
            _ => damage,
        }
    }

    pub fn dragon_phase_on_crystal_destroyed(&mut self, dragon: EntityId, phase: EnderDragonPhase, player: Option<EntityId>) {
        if phase == EnderDragonPhase::HoldingPattern {
            self.dragon_holding_pattern_on_crystal_destroyed(dragon, player);
        }
    }
}
