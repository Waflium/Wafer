use wafer_core::Vec3;

use super::abstract_chested_horse::{self, AbstractChestedHorse};
use crate::server::level::ServerLevel;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityDimensions, EntityId};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.5).with_eye_height(1.425).with_passenger_attachments(&[Vec3::new(0.0, 1.1125f32 as f64, 0.0)]);
const BABY_PASSENGER_ATTACHMENTS: &[Vec3] = &[Vec3::new(0.0, (1.5f32 + 0.03125f32) as f64, -0.3125f32 as f64)];

#[derive(Default)]
pub struct Donkey {
    pub abstract_chested_horse: AbstractChestedHorse,
}

pub fn baby_dimensions() -> EntityDimensions {
    abstract_chested_horse::baby_dimensions(DIMENSIONS, BABY_PASSENGER_ATTACHMENTS)
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.donkey.ambient"
}

pub fn get_angry_sound() -> &'static str {
    "minecraft:entity.donkey.angry"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.donkey.death"
}

pub fn get_eating_sound() -> &'static str {
    "minecraft:entity.donkey.eat"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.donkey.hurt"
}

impl Donkey {
    pub fn create() -> Mob {
        AbstractChestedHorse::create("minecraft:donkey", DIMENSIONS, MobKind::Donkey(Donkey::default()))
    }
}

impl ServerLevel {
    pub fn is_donkey(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Donkey(_)))
    }

    pub fn donkey_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        if partner == id {
            return false;
        }
        if !self.is_donkey(partner) && !self.is_horse(partner) { false } else { self.can_parent(id) && self.can_parent(partner) }
    }

    pub fn donkey_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(if self.is_horse(partner) { super::mule::Mule::create() } else { Donkey::create() });
        self.set_offspring_attributes(id, partner, &mut baby);
        Some(baby)
    }
}
