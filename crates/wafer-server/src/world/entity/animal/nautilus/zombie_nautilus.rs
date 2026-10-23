use std::sync::OnceLock;

use wafer_data::{EquipmentSlot, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_nautilus::{AbstractNautilus, abstract_nautilus_defaults};
use super::zombie_nautilus_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::ZOMBIE_NAUTILUS_ATTRIBUTES;
use crate::world::entity::data::DataValue;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::tamable_animal::TamableAnimal;
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityId};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DATA_VARIANT_ID: usize = 21;
pub const VARIANT_REGISTRY: &str = "minecraft:zombie_nautilus_variant";

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[memory_module_type::ANGRY_AT.erase(), memory_module_type::ATTACK_TARGET_COOLDOWN.erase()],
    &[SensorType::NearestLivingEntities, SensorType::NearestAdult, SensorType::NearestPlayers, SensorType::HurtBy, SensorType::NautilusTemptations],
    |_| zombie_nautilus_ai::get_activities(),
);

pub struct ZombieNautilus;

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, crate::core::data_registries::json_entries("minecraft:zombie_nautilus_variant")))
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::ZombieNautilusVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::ZombieNautilusVariant(variant));
}

pub fn sun_protection_slot() -> EquipmentSlot {
    EquipmentSlot::Body
}

pub fn get_ambient_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.zombie_nautilus.ambient" } else { "minecraft:entity.zombie_nautilus.ambient_land" }
}

pub fn get_hurt_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.zombie_nautilus.hurt" } else { "minecraft:entity.zombie_nautilus.hurt_land" }
}

pub fn get_death_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.zombie_nautilus.death" } else { "minecraft:entity.zombie_nautilus.death_land" }
}

pub fn get_dash_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.zombie_nautilus.dash" } else { "minecraft:entity.zombie_nautilus.dash_land" }
}

pub fn get_dash_ready_sound(entity: &Entity) -> &'static str {
    if entity.is_under_water() { "minecraft:entity.zombie_nautilus.dash_ready" } else { "minecraft:entity.zombie_nautilus.dash_ready_land" }
}

pub fn get_eating_sound() -> &'static str {
    "minecraft:entity.zombie_nautilus.eat"
}

pub fn get_swim_sound() -> &'static str {
    "minecraft:entity.zombie_nautilus.swim"
}

pub fn can_be_a_baby() -> bool {
    false
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    let pos = mob.entity.block_position();
    if let Some(variant) = variant_utils::select_variant_to_spawn(level, pos, spawn_selectors()) {
        set_variant(&mut mob.entity, variant);
    }
}

impl ZombieNautilus {
    pub fn create() -> Mob {
        let mut data = abstract_nautilus_defaults();
        data.push(DataValue::ZombieNautilusVariant(variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:temperate").unwrap_or(0)));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:zombie_nautilus"), super::nautilus::DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&ZOMBIE_NAUTILUS_ATTRIBUTES), MobKind::ZombieNautilus(AbstractNautilus::default()));
        AbstractNautilus::init(&mut mob);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        TamableAnimal::add_additional_save_data(mob, output);
        output.push(("variant".into(), Nbt::String(variant_utils::variant_name(VARIANT_REGISTRY, get_variant(&mob.entity)).into())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        TamableAnimal::read_additional_save_data(mob, input);
        if let Some(variant) = input.get("variant").and_then(Nbt::as_str).and_then(|name| variant_utils::read_variant(VARIANT_REGISTRY, name)) {
            set_variant(&mut mob.entity, variant);
        }
    }
}

impl ServerLevel {
    pub fn is_zombie_nautilus(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::ZombieNautilus(_)))
    }

    pub fn zombie_nautilus_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        zombie_nautilus_ai::update_activity(self.get_brain_mut(id));
        self.abstract_nautilus_custom_server_ai_step(id);
    }

    pub fn zombie_nautilus_play_eating_sound(&mut self, id: EntityId) {
        self.make_sound(id, crate::world::entity::sound_event(get_eating_sound()));
    }

    pub fn zombie_nautilus_can_be_leashed(&self, id: EntityId) -> bool {
        !self.abstract_nautilus_is_aggravated(id) && !self.abstract_nautilus_is_mob_controlled(id)
    }
}
