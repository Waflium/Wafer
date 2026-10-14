use wafer_data::mob_effects;

use super::abstract_skeleton::AbstractSkeleton;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::SKELETON_ATTRIBUTES;
use crate::world::entity::data::mob_defaults;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::item::ItemStack;

#[derive(Default)]
pub struct Stray {
    pub abstract_skeleton: AbstractSkeleton,
}

impl Stray {
    pub fn create() -> Mob {
        AbstractSkeleton::create_with("minecraft:stray", &SKELETON_ATTRIBUTES, mob_defaults(), MobKind::Stray(Stray::default()))
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.stray.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.stray.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.stray.death"
    }

    pub fn get_step_sound() -> &'static str {
        "minecraft:entity.stray.step"
    }
}

impl ServerLevel {
    pub fn stray_get_arrow(&mut self, id: EntityId, projectile: &ItemStack, power: f32, firing_weapon: Option<&ItemStack>) -> Projectile {
        let mut arrow = self.abstract_skeleton_get_arrow(id, projectile, power, firing_weapon);
        if let ProjectileKind::Arrow(_) = arrow.kind {
            arrow.arrow_add_effect(&MobEffectInstance::new(mob_effects::SLOWNESS, 600, 0));
        }
        arrow
    }
}
