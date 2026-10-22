#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnderDragonPhase {
    HoldingPattern,
    StrafePlayer,
    LandingApproach,
    Landing,
    Takeoff,
    SittingFlaming,
    SittingScanning,
    SittingAttacking,
    ChargingPlayer,
    Dying,
    Hovering,
}

impl EnderDragonPhase {
    pub const VALUES: [EnderDragonPhase; 11] = [
        EnderDragonPhase::HoldingPattern,
        EnderDragonPhase::StrafePlayer,
        EnderDragonPhase::LandingApproach,
        EnderDragonPhase::Landing,
        EnderDragonPhase::Takeoff,
        EnderDragonPhase::SittingFlaming,
        EnderDragonPhase::SittingScanning,
        EnderDragonPhase::SittingAttacking,
        EnderDragonPhase::ChargingPlayer,
        EnderDragonPhase::Dying,
        EnderDragonPhase::Hovering,
    ];

    pub fn get_id(self) -> i32 {
        self as i32
    }

    pub fn get_by_id(id: i32) -> EnderDragonPhase {
        if id >= 0 && (id as usize) < EnderDragonPhase::VALUES.len() { EnderDragonPhase::VALUES[id as usize] } else { EnderDragonPhase::HoldingPattern }
    }
}
