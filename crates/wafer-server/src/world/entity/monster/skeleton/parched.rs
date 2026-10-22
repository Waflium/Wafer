use wafer_data::mob_effects;

use super::abstract_skeleton::{AbstractSkeleton, INCREASED_HARD_ATTACK_INTERVAL, INCREASED_NORMAL_ATTACK_INTERVAL};
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::PARCHED_ATTRIBUTES;
use crate::world::entity::data::mob_defaults;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::item::ItemStack;

#[derive(Default)]
pub struct Parched {
    pub abstract_skeleton: AbstractSkeleton,
}

impl Parched {
    pub fn create() -> Mob {
        AbstractSkeleton::create_with("minecraft:parched", &PARCHED_ATTRIBUTES, mob_defaults(), MobKind::Parched(Parched::default()))
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.parched.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.parched.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.parched.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.parched.step"
    }

    pub fn get_hard_attack_interval() -> i32 {
        INCREASED_HARD_ATTACK_INTERVAL
    }

    pub fn get_attack_interval() -> i32 {
        INCREASED_NORMAL_ATTACK_INTERVAL
    }
}

impl ServerLevel {
    pub fn parched_get_arrow(&mut self, id: EntityId, projectile: &ItemStack, power: f32, firing_weapon: Option<&ItemStack>) -> Projectile {
        let mut arrow = self.abstract_skeleton_get_arrow(id, projectile, power, firing_weapon);
        if let ProjectileKind::Arrow(_) = arrow.kind {
            arrow.arrow_add_effect(&MobEffectInstance::new(mob_effects::WEAKNESS, 600, 0));
        }
        arrow
    }

    pub fn parched_can_be_affected(&self, id: EntityId, new_effect: &MobEffectInstance) -> bool {
        if new_effect.effect == mob_effects::WEAKNESS { false } else { self.living_entity_can_be_affected(id, new_effect) }
    }
}
