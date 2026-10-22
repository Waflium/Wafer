use wafer_core::Vec3;
use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::hoglin::hoglin_base;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::{
    behavior_utils, melee_attack, random_stroll, run_one, set_entity_look_target_sometimes, set_walk_target_from_attack_target_if_target_out_of_reach, set_walk_target_from_look_target,
    start_attacking, stop_attacking_if_target_invalid,
};
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;
use crate::world::entity::ai::sensing::sensor;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::{Attribute, ZOGLIN_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::schedule::Activity;
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_types, sound_event};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

const DATA_BABY_ID: usize = 16;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.4).with_passenger_attachments(&[Vec3::new(0.0, 1.49375, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.75, 0.85).with_eye_height(0.625).with_passenger_attachments(&[Vec3::new(0.0, 0.875, 0.0)]);
const BABY_ATTACK_DAMAGE: f64 = 0.5;
const ATTACK_INTERVAL: i32 = 40;
const BABY_ATTACK_INTERVAL: i32 = 15;
const ATTACK_DURATION: i64 = 200;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 0.4;
const DEFAULT_BABY: bool = false;

pub static BRAIN_PROVIDER: Provider = brain::provider(&[SensorType::NearestLivingEntities, SensorType::NearestPlayers], |_| get_activities());

#[derive(Default)]
pub struct Zoglin {
    attack_animation_remaining_ticks: i32,
}

fn zoglin_mut(mob: &mut Mob) -> &mut Zoglin {
    match &mut mob.kind {
        MobKind::Zoglin(zoglin) => zoglin,
        _ => unreachable!("a zoglin"),
    }
}

pub fn is_baby(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_BABY_ID), DataValue::Boolean(true))
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity(), init_fight_activity()]
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(Activity::CORE, 0, vec![Box::new(LookAtTargetSink::new(45, 90)), Box::new(MoveToTargetSink::default())])
}

fn init_idle_activity() -> ActivityData {
    let behaviors: Vec<(Box<dyn BehaviorControl>, i32)> = vec![
        (Box::new(random_stroll::stroll(SPEED_MULTIPLIER_WHEN_IDLING)), 2),
        (Box::new(set_walk_target_from_look_target::create(SPEED_MULTIPLIER_WHEN_IDLING, 3)), 2),
        (Box::new(DoNothing::new(30, 60)), 1),
    ];
    ActivityData::create(
        Activity::IDLE,
        10,
        vec![
            Box::new(start_attacking::create(find_nearest_valid_attack_target)),
            Box::new(set_entity_look_target_sometimes::create(8.0, IntProvider::Uniform { min_inclusive: 30, max_inclusive: 60 })),
            Box::new(run_one::new(behaviors)),
        ],
    )
}

fn init_fight_activity() -> ActivityData {
    ActivityData::create_with_memory(
        Activity::FIGHT,
        10,
        vec![
            Box::new(set_walk_target_from_attack_target_if_target_out_of_reach::create_with(|_, _| 1.0)),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(|level, body| !level.is_baby(body)), melee_attack::create(ATTACK_INTERVAL))),
            Box::new(behavior_builder::sequence(behavior_builder::trigger_if(|level, body| level.is_baby(body)), melee_attack::create(BABY_ATTACK_INTERVAL))),
            Box::new(stop_attacking_if_target_invalid::create()),
        ],
        memory_module_type::ATTACK_TARGET.erase(),
    )
}

fn find_nearest_valid_attack_target(level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
    let nearest_visible_living_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned().unwrap_or_else(NearestVisibleLivingEntities::empty);
    nearest_visible_living_entities.find_closest(level, |level, target| {
        let target_type = level.entity(target).entity_type;
        target_type != entity_types::ZOGLIN.id && target_type != entity_types::CREEPER.id && sensor::is_entity_attackable(level, body, target)
    })
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    if level.get_random().next_float() < hoglin_base::PROBABILITY_OF_SPAWNING_AS_BABY {
        set_baby(mob, true);
    }
}

pub fn set_baby(mob: &mut Mob, baby: bool) {
    let changed = is_baby(&mob.entity) != baby;
    mob.entity.data.set(DATA_BABY_ID, DataValue::Boolean(baby));
    if changed {
        mob.refresh_dimensions();
    }
    if baby {
        mob.living.attributes.instance(Attribute::AttackDamage).expect("zoglins have an attack damage").set_base_value(BABY_ATTACK_DAMAGE);
    }
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.zoglin.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.zoglin.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.zoglin.step"
}

impl Zoglin {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(DEFAULT_BABY));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:zoglin"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&ZOGLIN_ATTRIBUTES), MobKind::Zoglin(Zoglin::default()));
        mob.xp_reward = 5;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("IsBaby".into(), Nbt::Byte(is_baby(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let baby = crate::world::level::block::entity::byte_or(nbt, "IsBaby", DEFAULT_BABY as i8) != 0;
        set_baby(mob, baby);
    }
}

impl ServerLevel {
    pub fn zoglin_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        if !self.is_living(target) {
            return false;
        }
        zoglin_mut(self.mob_mut(id).expect("a zoglin")).attack_animation_remaining_ticks = 10;
        self.broadcast_entity_event(id, 4);
        self.make_sound(id, sound_event("minecraft:entity.zoglin.attack"));
        hoglin_base::hurt_and_throw_target(self, id, target)
    }

    pub fn zoglin_blocked_by_item(&mut self, id: EntityId, defender: EntityId) {
        if !self.is_baby(id) {
            hoglin_base::throw_target(self, id, defender);
        }
    }

    pub fn zoglin_hurt_server(&mut self, id: EntityId, source: crate::world::damagesource::DamageSource, damage: f32) -> bool {
        let source_entity = source.causing_entity;
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if !was_hurt {
            return false;
        }
        let Some(attacker) = source_entity.filter(|&entity| self.is_living(entity)) else { return true };
        if self.can_attack(id, attacker) && !behavior_utils::is_other_target_much_further_away_than_current_attack_target(self, id, attacker, 4.0) {
            let brain = self.get_brain_mut(id);
            brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
            brain.set_memory_with_expiry(memory_module_type::ATTACK_TARGET, attacker, ATTACK_DURATION);
        }
        true
    }

    fn zoglin_update_activity(&mut self, id: EntityId) {
        let brain = self.get_brain_mut(id);
        let old_activity = brain.get_active_non_core_activity();
        brain.set_active_activity_to_first_valid(&[Activity::FIGHT, Activity::IDLE]);
        let new_activity = brain.get_active_non_core_activity();
        if new_activity == Some(Activity::FIGHT) && old_activity != Some(Activity::FIGHT) {
            self.make_sound(id, sound_event("minecraft:entity.zoglin.angry"));
        }
        let aggressive = self.get_brain(id).has_memory_value(memory_module_type::ATTACK_TARGET);
        self.mob_mut(id).expect("a zoglin").set_aggressive(aggressive);
    }

    pub fn zoglin_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        self.zoglin_update_activity(id);
    }

    pub fn zoglin_ai_step(&mut self, id: EntityId) {
        let zoglin = zoglin_mut(self.mob_mut(id).expect("a zoglin"));
        if zoglin.attack_animation_remaining_ticks > 0 {
            zoglin.attack_animation_remaining_ticks -= 1;
        }
        self.monster_ai_step(id);
    }

    pub fn zoglin_get_ambient_sound(&self, id: EntityId) -> &'static str {
        if self.get_brain(id).has_memory_value(memory_module_type::ATTACK_TARGET) { "minecraft:entity.zoglin.angry" } else { "minecraft:entity.zoglin.ambient" }
    }
}
