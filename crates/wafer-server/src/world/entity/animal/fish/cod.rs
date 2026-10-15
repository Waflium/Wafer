use super::abstract_fish::{AbstractFish, abstract_fish_defaults};
use super::abstract_schooling_fish::AbstractSchoolingFish;
use crate::world::entity::EntityDimensions;
use crate::world::entity::mob::{Mob, MobKind};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.3).with_eye_height(0.195);

#[derive(Default)]
pub struct Cod {
    pub abstract_schooling_fish: AbstractSchoolingFish,
}

impl Cod {
    pub fn create() -> Mob {
        AbstractFish::create_with("minecraft:cod", DIMENSIONS, abstract_fish_defaults(), MobKind::Cod(Cod::default()))
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.cod.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.cod.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.cod.hurt"
    }

    pub fn get_flop_sound() -> &'static str {
        "minecraft:entity.cod.flop"
    }
}
