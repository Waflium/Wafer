use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::mob::MobKind;

impl ServerLevel {
    pub fn perform_ranged_attack(&mut self, id: EntityId, target: EntityId, power: f32) {
        if self.is_drowned(id) {
            self.drowned_perform_ranged_attack(id, target, power);
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Witch(_))) {
            self.witch_perform_ranged_attack(id, target, power);
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::SnowGolem(_))) {
            self.snow_golem_perform_ranged_attack(id, target, power);
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Pillager(_))) {
            self.pillager_perform_ranged_attack(id);
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Illusioner(_))) {
            self.illusioner_perform_ranged_attack(id, target, power);
        } else if self.is_llama(id) {
            self.llama_perform_ranged_attack(id, target);
        } else {
            self.abstract_skeleton_perform_ranged_attack(id, target, power);
        }
    }

    pub fn ranged_attack_uncertainty(&self, id: EntityId) -> f32 {
        if self.is_llama(id) {
            self.llama_ranged_attack_uncertainty()
        } else if self.is_drowned(id) {
            self.drowned_ranged_attack_uncertainty(id)
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Witch(_))) {
            self.witch_ranged_attack_uncertainty()
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::SnowGolem(_))) {
            self.snow_golem_ranged_attack_uncertainty()
        } else {
            self.ranged_attack_mob_ranged_attack_uncertainty()
        }
    }

    pub fn ranged_attack_mob_ranged_attack_uncertainty(&self) -> f32 {
        (14 - self.difficulty as i32 * 4) as f32
    }
}
