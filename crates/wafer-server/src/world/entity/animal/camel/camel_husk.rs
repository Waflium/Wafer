use wafer_data::item_tags;

use super::camel::Camel;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::sound_event;
use crate::world::item::ItemStack;

pub struct CamelHusk;

impl CamelHusk {
    pub fn create() -> Mob {
        Camel::create_with("minecraft:camel_husk", MobKind::CamelHusk)
    }
}

pub fn remove_when_far_away() -> bool {
    true
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::CAMEL_HUSK_FOOD)
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.camel_husk.ambient"
}

pub fn can_mate() -> bool {
    false
}

pub fn can_fall_in_love() -> bool {
    false
}

pub fn can_be_a_baby() -> bool {
    false
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.camel_husk.death"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.camel_husk.hurt"
}

pub fn play_step_sound(level: &mut ServerLevel, id: EntityId, sand: bool) {
    let sound = if sand { "minecraft:entity.camel_husk.step_sand" } else { "minecraft:entity.camel_husk.step" };
    level.entity_play_sound(id, sound_event(sound), 0.4, 1.0);
}

pub fn get_dashing_sound() -> &'static str {
    "minecraft:entity.camel_husk.dash"
}

pub fn get_dash_ready_sound() -> &'static str {
    "minecraft:entity.camel_husk.dash_ready"
}

pub fn get_eating_sound() -> &'static str {
    "minecraft:entity.camel_husk.eat"
}

pub fn get_stand_up_sound() -> &'static str {
    "minecraft:entity.camel_husk.stand"
}

pub fn get_sit_down_sound() -> &'static str {
    "minecraft:entity.camel_husk.sit"
}

pub fn get_saddle_sound() -> &'static str {
    "minecraft:entity.camel_husk.saddle"
}

pub fn charge_speed_modifier() -> f32 {
    4.0
}

impl ServerLevel {
    pub fn is_camel_husk(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::CamelHusk(_)))
    }

    pub fn camel_husk_is_mob_controlled(&self, id: EntityId) -> bool {
        self.first_passenger(id).is_some_and(|passenger| self.is_mob(passenger))
    }

    pub fn camel_husk_can_be_leashed(&self, id: EntityId) -> bool {
        !self.camel_husk_is_mob_controlled(id)
    }
}
