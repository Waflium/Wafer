use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

impl ServerLevel {
    pub fn perform_ranged_attack(&mut self, id: EntityId, target: EntityId, power: f32) {
        if self.is_drowned(id) {
            self.drowned_perform_ranged_attack(id, target, power);
        } else {
            self.abstract_skeleton_perform_ranged_attack(id, target, power);
        }
    }

    pub fn ranged_attack_uncertainty(&self, id: EntityId) -> f32 {
        if self.is_drowned(id) { self.drowned_ranged_attack_uncertainty(id) } else { self.ranged_attack_mob_ranged_attack_uncertainty() }
    }

    pub fn ranged_attack_mob_ranged_attack_uncertainty(&self) -> f32 {
        (14 - self.difficulty as i32 * 4) as f32
    }
}
