use wafer_data::{EquipmentSlot, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_piglin::{self, AbstractPiglin};
use super::{piglin, piglin_brute_ai};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::PIGLIN_BRUTE_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;

pub const DIMENSIONS: EntityDimensions = piglin::DIMENSIONS;

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[memory_module_type::NEAREST_VISIBLE_ADULT_PIGLINS.erase()],
    &[SensorType::NearestLivingEntities, SensorType::NearestPlayers, SensorType::NearestItems, SensorType::HurtBy, SensorType::PiglinBruteSpecificSensor],
    |_| piglin_brute_ai::get_activities(),
);

#[derive(Default)]
pub struct PiglinBrute {
    pub abstract_piglin: AbstractPiglin,
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.piglin_brute.ambient"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.piglin_brute.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.piglin_brute.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.piglin_brute.step"
}

impl PiglinBrute {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:piglin_brute"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&PIGLIN_BRUTE_ATTRIBUTES), MobKind::PiglinBrute(PiglinBrute::default()));
        abstract_piglin::init(&mut mob);
        mob.xp_reward = 20;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        abstract_piglin::add_additional_save_data(mob, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        abstract_piglin::read_additional_save_data(mob, nbt);
    }
}

impl ServerLevel {
    pub fn piglin_brute_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        piglin_brute_ai::init_memories(self, id);
        self.piglin_brute_populate_default_equipment_slots(id);
        self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data)
    }

    fn piglin_brute_populate_default_equipment_slots(&mut self, id: EntityId) {
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::GOLDEN_AXE, 1));
    }

    pub fn piglin_brute_wants_to_pick_up(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        item_stack.is(items::GOLDEN_AXE) && self.mob_wants_to_pick_up(id, item_stack)
    }

    pub fn piglin_brute_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick_in_zone(self, id, "piglinBruteBrain");
        if self.mob(id).is_none() {
            return;
        }
        piglin_brute_ai::update_activity(self, id);
        piglin_brute_ai::maybe_play_activity_sound(self, id);
        self.abstract_piglin_custom_server_ai_step(id);
    }

    pub fn piglin_brute_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let source_entity = source.causing_entity;
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if was_hurt && let Some(source_entity) = source_entity.filter(|&entity| self.is_living(entity)) {
            piglin_brute_ai::was_hurt_by(self, id, source_entity);
        }
        was_hurt
    }

    pub fn piglin_brute_play_angry_sound(&mut self, id: EntityId) {
        self.make_sound(id, sound_event("minecraft:entity.piglin_brute.angry"));
    }
}
