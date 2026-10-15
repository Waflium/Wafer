use wafer_core::Vec3;

use super::abstract_chested_horse::{self, AbstractChestedHorse};
use crate::server::level::ServerLevel;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityDimensions, EntityId};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.6).with_eye_height(1.52).with_passenger_attachments(&[Vec3::new(0.0, 1.2125f32 as f64, 0.0)]);
const BABY_PASSENGER_ATTACHMENTS: &[Vec3] = &[Vec3::new(0.0, (1.6f32 + 0.03125f32) as f64, -0.3125f32 as f64)];

#[derive(Default)]
pub struct Mule {
    pub abstract_chested_horse: AbstractChestedHorse,
}

pub fn baby_dimensions() -> EntityDimensions {
    abstract_chested_horse::baby_dimensions(DIMENSIONS, BABY_PASSENGER_ATTACHMENTS)
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.mule.ambient"
}

pub fn get_angry_sound() -> &'static str {
    "minecraft:entity.mule.angry"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.mule.death"
}

pub fn get_eating_sound() -> &'static str {
    "minecraft:entity.mule.eat"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.mule.hurt"
}

impl Mule {
    pub fn create() -> Mob {
        AbstractChestedHorse::create("minecraft:mule", DIMENSIONS, MobKind::Mule(Mule::default()))
    }
}

impl ServerLevel {
    pub fn is_mule(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Mule(_)))
    }

    pub fn mule_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Mule::create()))
    }
}
