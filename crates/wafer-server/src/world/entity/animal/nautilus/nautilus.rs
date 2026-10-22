use wafer_core::Vec3;
use wafer_data::{damage_types, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_nautilus::{AbstractNautilus, abstract_nautilus_defaults};
use super::nautilus_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::NAUTILUS_ATTRIBUTES;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::tamable_animal::{self, TamableAnimal};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};

const NAUTILUS_TOTAL_AIR_SUPPLY: i32 = 300;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.875, 0.95).with_eye_height(0.2751).with_passenger_attachments(&[Vec3::new(0.0, 1.1375f32 as f64, 0.0)]);

const BABY_PASSENGER_ATTACHMENTS: &[Vec3] = &[Vec3::new(0.0, 0.5, 0.0)];

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[memory_module_type::ANGRY_AT.erase(), memory_module_type::ATTACK_TARGET_COOLDOWN.erase()],
    &[SensorType::NearestLivingEntities, SensorType::NearestAdult, SensorType::NearestPlayers, SensorType::HurtBy, SensorType::NautilusTemptations],
    |_| nautilus_ai::get_activities(),
);

pub struct Nautilus;

pub fn baby_dimensions() -> EntityDimensions {
    DIMENSIONS.scale(0.5).with_passenger_attachments(BABY_PASSENGER_ATTACHMENTS)
}

pub fn get_ambient_sound(entity: &Entity, baby: bool) -> &'static str {
    match (baby, entity.is_under_water()) {
        (true, true) => "minecraft:entity.baby_nautilus.ambient",
        (true, false) => "minecraft:entity.baby_nautilus.ambient_land",
        (false, true) => "minecraft:entity.nautilus.ambient",
        (false, false) => "minecraft:entity.nautilus.ambient_land",
    }
}

pub fn get_hurt_sound(entity: &Entity, baby: bool) -> &'static str {
    match (baby, entity.is_under_water()) {
        (true, true) => "minecraft:entity.baby_nautilus.hurt",
        (true, false) => "minecraft:entity.baby_nautilus.hurt_land",
        (false, true) => "minecraft:entity.nautilus.hurt",
        (false, false) => "minecraft:entity.nautilus.hurt_land",
    }
}

pub fn get_death_sound(entity: &Entity, baby: bool) -> &'static str {
    match (baby, entity.is_under_water()) {
        (true, true) => "minecraft:entity.baby_nautilus.death",
        (true, false) => "minecraft:entity.baby_nautilus.death_land",
        (false, true) => "minecraft:entity.nautilus.death",
        (false, false) => "minecraft:entity.nautilus.death_land",
    }
}

pub fn get_dash_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.nautilus.dash" } else { "minecraft:entity.nautilus.dash_land" }
}

pub fn get_dash_ready_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.nautilus.dash_ready" } else { "minecraft:entity.nautilus.dash_ready_land" }
}

pub fn get_eating_sound(baby: bool) -> &'static str {
    if baby { "minecraft:entity.baby_nautilus.eat" } else { "minecraft:entity.nautilus.eat" }
}

pub fn get_swim_sound(baby: bool) -> &'static str {
    if baby { "minecraft:entity.baby_nautilus.swim" } else { "minecraft:entity.nautilus.swim" }
}

impl Nautilus {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:nautilus"), DIMENSIONS, abstract_nautilus_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&NAUTILUS_ATTRIBUTES), MobKind::Nautilus(AbstractNautilus::default()));
        AbstractNautilus::init(&mut mob);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        TamableAnimal::add_additional_save_data(mob, output);
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        TamableAnimal::read_additional_save_data(mob, input);
    }
}

impl ServerLevel {
    pub fn is_nautilus(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Nautilus(_)))
    }

    pub fn nautilus_get_breed_offspring(&mut self, id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Nautilus::create());
        if self.is_tame(id) {
            tamable_animal::set_owner_reference(&mut baby.entity, tamable_animal::get_owner_reference(self.entity(id)));
            tamable_animal::set_tame(&mut baby, true, true);
        }
        Some(baby)
    }

    pub fn nautilus_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        nautilus_ai::update_activity(self.get_brain_mut(id));
        self.abstract_nautilus_custom_server_ai_step(id);
    }

    pub fn nautilus_play_eating_sound(&mut self, id: EntityId) {
        let sound = get_eating_sound(self.is_baby(id));
        self.make_sound(id, sound_event(sound));
    }

    fn nautilus_handle_air_supply(&mut self, id: EntityId, pre_tick_air_supply: i32) {
        if self.is_alive(id) && !self.entity(id).is_in_water() {
            self.entity_mut(id).set_air_supply(pre_tick_air_supply - 1);
            if self.entity(id).air_supply() <= -20 {
                self.entity_mut(id).set_air_supply(0);
                self.hurt_server(id, DamageSource::new(damage_types::DRY_OUT), 2.0);
            }
        } else {
            self.entity_mut(id).set_air_supply(NAUTILUS_TOTAL_AIR_SUPPLY);
        }
    }

    pub fn nautilus_base_tick(&mut self, id: EntityId) {
        let air_supply = self.entity(id).air_supply();
        self.mob_base_tick(id);
        if self.mob(id).is_some_and(|mob| !mob.is_no_ai()) {
            self.nautilus_handle_air_supply(id, air_supply);
        }
    }

    pub fn nautilus_can_be_leashed(&self, id: EntityId) -> bool {
        !self.abstract_nautilus_is_aggravated(id)
    }
}
