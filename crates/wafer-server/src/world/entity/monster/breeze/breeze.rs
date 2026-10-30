use wafer_core::Vec3;

use crate::util::profiling::profiler;
use wafer_core::random::RandomSource;
use wafer_data::{registry, registry_id};

use super::breeze_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::BREEZE_ATTRIBUTES;
use crate::world::entity::data::{Pose, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile_deflection::ProjectileDeflection;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_types, sound_event};
use crate::world::level::pathfinder::path_type::PathType;

const SLIDE_PARTICLES_AMOUNT: i32 = 20;
const IDLE_PARTICLES_AMOUNT: i32 = 1;
const FALL_DISTANCE_SOUND_TRIGGER_THRESHOLD: f32 = 3.0;
const WHIRL_SOUND_FREQUENCY_MIN: i32 = 1;
const WHIRL_SOUND_FREQUENCY_MAX: i32 = 80;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.77).with_eye_height(1.3452);

pub static BRAIN_PROVIDER: Provider =
    brain::provider(&[SensorType::NearestLivingEntities, SensorType::HurtBy, SensorType::NearestPlayers, SensorType::BreezeAttackEntitySensor], |_| breeze_ai::get_activities());

#[derive(Default)]
pub struct Breeze {
    jump_trail_started_tick: i32,
    sound_tick: i32,
}

fn breeze_mut(mob: &mut Mob) -> &mut Breeze {
    match &mut mob.kind {
        MobKind::Breeze(breeze) => breeze,
        _ => unreachable!("a breeze"),
    }
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.breeze.death"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.breeze.hurt"
}

pub fn get_max_head_y_rot() -> i32 {
    30
}

pub fn get_head_rot_speed() -> i32 {
    25
}

pub fn get_ambient_sound(mob: &Mob) -> &'static str {
    if mob.entity.on_ground { "minecraft:entity.breeze.idle_ground" } else { "minecraft:entity.breeze.idle_air" }
}

impl Breeze {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:breeze"), DIMENSIONS, mob_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&BREEZE_ATTRIBUTES), MobKind::Breeze(Breeze::default()));
        mob.set_pathfinding_malus(PathType::OnTopOfTrapdoor, -1.0);
        mob.set_pathfinding_malus(PathType::Fire, -1.0);
        mob.xp_reward = 10;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        let mut brain = BRAIN_PROVIDER.make_brain(mob, packed_brain);
        brain.set_default_activity(Activity::FIGHT);
        brain.use_default_activity();
        brain
    }
}

impl ServerLevel {
    pub fn breeze_tick(&mut self, id: EntityId) {
        match self.entity(id).pose() {
            Pose::Shooting | Pose::Inhaling | Pose::Standing => {
                self.breeze_reset_jump_trail(id);
                let amount = IDLE_PARTICLES_AMOUNT + self.entity_mut(id).random.next_int_bounded(1);
                self.breeze_emit_ground_particles(id, amount);
            }
            Pose::Sliding => self.breeze_emit_ground_particles(id, SLIDE_PARTICLES_AMOUNT),
            Pose::LongJumping => self.breeze_emit_jump_trail_particles(id),
            _ => {}
        }
        let mob = self.mob_mut(id).expect("a breeze");
        let sound_tick = breeze_mut(mob).sound_tick;
        let sound_tick = if sound_tick == 0 { mob.entity.random.next_int_bounded(WHIRL_SOUND_FREQUENCY_MAX - WHIRL_SOUND_FREQUENCY_MIN + 1) + WHIRL_SOUND_FREQUENCY_MIN } else { sound_tick - 1 };
        breeze_mut(mob).sound_tick = sound_tick;
        if sound_tick == 0 {
            self.breeze_play_whirl_sound(id);
        }
        self.mob_tick(id);
    }

    fn breeze_reset_jump_trail(&mut self, id: EntityId) {
        breeze_mut(self.mob_mut(id).expect("a breeze")).jump_trail_started_tick = 0;
    }

    fn breeze_emit_jump_trail_particles(&mut self, id: EntityId) {
        breeze_mut(self.mob_mut(id).expect("a breeze")).jump_trail_started_tick += 1;
    }

    fn breeze_emit_ground_particles(&self, _id: EntityId, _amount: i32) {}

    fn breeze_play_whirl_sound(&mut self, id: EntityId) {
        let random = &mut self.entity_mut(id).random;
        random.next_float();
        random.next_float();
    }

    pub fn breeze_deflection(&self, id: EntityId, projectile: EntityId) -> ProjectileDeflection {
        if self.is_wind_charge(projectile) {
            ProjectileDeflection::None
        } else if wafer_data::entity_type_tags::DEFLECTS_PROJECTILES.contains(self.entity(id).entity_type as usize) {
            ProjectileDeflection::Breeze
        } else {
            ProjectileDeflection::None
        }
    }

    pub fn breeze_get_hurt_by(&self, id: EntityId) -> Option<EntityId> {
        self.get_brain(id).get_memory(memory_module_type::HURT_BY).and_then(|source| source.causing_entity).filter(|&entity| self.is_living(entity))
    }

    pub fn breeze_within_inner_circle_range(&self, id: EntityId, target: Vec3) -> bool {
        let our_position = Vec3::at_center_of(self.entity(id).block_position());
        target.closer_than_xz_y(our_position, 4.0, 10.0)
    }

    pub fn breeze_custom_server_ai_step(&mut self, id: EntityId) {
        profiler::push("breezeBrain");
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            profiler::pop();
            return;
        }
        profiler::pop_push("breezeActivityUpdate");
        breeze_ai::update_activity(self, id);
        profiler::pop();
    }

    pub fn breeze_can_attack(&self, id: EntityId, target: EntityId) -> bool {
        let target_type = self.entity(target).entity_type;
        (target_type == entity_types::PLAYER.id || target_type == entity_types::IRON_GOLEM.id) && self.mob_can_attack(id, target)
    }

    pub fn breeze_get_firing_y_position(&self, id: EntityId) -> f64 {
        let e = self.entity(id);
        e.y() + (e.dimensions.height / 2.0) as f64 + 0.3f32 as f64
    }

    pub fn breeze_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        source.causing_entity.is_some_and(|entity| self.entity_ref(entity).is_some_and(|e| e.entity_type == entity_types::BREEZE.id)) || self.living_entity_is_invulnerable_to(id, source)
    }

    pub fn breeze_on_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64) {
        if fall_distance > FALL_DISTANCE_SOUND_TRIGGER_THRESHOLD as f64 {
            self.entity_play_sound(id, sound_event("minecraft:entity.breeze.land"), 1.0, 1.0);
        }
    }
}
