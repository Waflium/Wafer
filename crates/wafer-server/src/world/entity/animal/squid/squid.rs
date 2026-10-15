use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{fluid_tags, mob_effects, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::{self, AgeableMobGroupData};
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::animal::ageable_water_creature::AgeableWaterCreature;
use crate::world::entity::attributes::SQUID_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_event;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{self, Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, MoverType, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.8, 0.8).with_eye_height(0.4);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.5).with_eye_height(0.37);
const PI: f32 = std::f64::consts::PI as f32;
const TAU: f32 = (std::f64::consts::PI * 2.0) as f32;

#[derive(Default)]
pub struct Squid {
    pub ageable_water_creature: AgeableWaterCreature,
    pub x_body_rot: f32,
    pub x_body_rot_o: f32,
    pub z_body_rot: f32,
    pub z_body_rot_o: f32,
    pub tentacle_movement: f32,
    pub old_tentacle_movement: f32,
    pub tentacle_angle: f32,
    pub old_tentacle_angle: f32,
    pub tentacle_speed: f32,
    pub rotate_speed: f32,
    pub movement_vector: Vec3,
}

pub fn squid(mob: &Mob) -> &Squid {
    match &mob.kind {
        MobKind::Squid(squid) => squid,
        MobKind::GlowSquid(glow_squid) => &glow_squid.squid,
        _ => unreachable!("not a squid"),
    }
}

pub fn squid_mut(mob: &mut Mob) -> &mut Squid {
    match &mut mob.kind {
        MobKind::Squid(squid) => squid,
        MobKind::GlowSquid(glow_squid) => &mut glow_squid.squid,
        _ => unreachable!("not a squid"),
    }
}

impl Squid {
    pub fn create() -> Mob {
        Squid::create_with("minecraft:squid", ageable_mob_defaults(), MobKind::Squid(Squid::default()))
    }

    pub fn create_with(entity_type: &str, defaults: Vec<DataValue>, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, defaults);
        let mut mob = Mob::new(entity, LivingEntity::new(&SQUID_ATTRIBUTES), kind);
        AgeableWaterCreature::init(&mut mob);
        let tentacle_speed = 1.0 / (mob.entity.random.next_float() + 1.0) * 0.2;
        squid_mut(&mut mob).tentacle_speed = tentacle_speed;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(0, Goal::SquidRandomMovement(SquidRandomMovementGoal));
        ai.goal_selector.add_goal(1, Goal::SquidFlee(SquidFleeGoal::default()));
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.squid.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.squid.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.squid.death"
    }

    pub fn get_squirt_sound() -> &'static str {
        "minecraft:entity.squid.squirt"
    }

    pub fn get_sound_volume() -> f32 {
        0.4
    }

    pub fn get_default_gravity() -> f64 {
        0.08
    }

    pub fn get_ink_particle() -> &'static str {
        "minecraft:squid_ink"
    }

    pub fn has_movement_vector(&self) -> bool {
        self.movement_vector.length_sqr() > 1.0e-5f32 as f64
    }

    fn rotate_vector(&self, y_body_rot_o: f32, vec: Vec3) -> Vec3 {
        let v = vec.x_rot(self.x_body_rot_o * (std::f64::consts::PI / 180.0) as f32);
        v.y_rot(-y_body_rot_o * (std::f64::consts::PI / 180.0) as f32)
    }
}

impl Mob {
    pub fn is_squid(&self) -> bool {
        matches!(self.kind, MobKind::Squid(_) | MobKind::GlowSquid(_))
    }

    pub fn get_squirt_sound(&self) -> &'static str {
        match self.kind {
            MobKind::GlowSquid(_) => super::glow_squid::GlowSquid::get_squirt_sound(),
            _ => Squid::get_squirt_sound(),
        }
    }

    pub fn get_ink_particle(&self) -> &'static str {
        match self.kind {
            MobKind::GlowSquid(_) => super::glow_squid::GlowSquid::get_ink_particle(),
            _ => Squid::get_ink_particle(),
        }
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let spawn_group_data = group_data.or(Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 0.05))));
    let group_data = ageable_mob::finalize_spawn(level, mob, spawn_group_data);
    mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

impl ServerLevel {
    pub fn squid_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Squid::create()))
    }

    pub fn squid_ai_step(&mut self, id: EntityId) {
        self.ageable_mob_ai_step(id);
        let in_water = self.entity(id).is_in_water();
        let authoritative = self.is_local_instance_authoritative(id);
        let gravity = self.gravity(id);
        let air_drag = self.air_drag(id);
        let mob = self.mob_mut(id).expect("a squid");
        let levitation = mob.living.effect(mob_effects::LEVITATION).map(|levitation| levitation.amplifier);
        let Mob { entity, living, kind, .. } = mob;
        let squid = match kind {
            MobKind::Squid(squid) => squid,
            MobKind::GlowSquid(glow_squid) => &mut glow_squid.squid,
            _ => unreachable!("not a squid"),
        };
        squid.x_body_rot_o = squid.x_body_rot;
        squid.z_body_rot_o = squid.z_body_rot;
        squid.old_tentacle_movement = squid.tentacle_movement;
        squid.old_tentacle_angle = squid.tentacle_angle;
        squid.tentacle_movement += squid.tentacle_speed;
        let mut anim_synch = false;
        if squid.tentacle_movement as f64 > std::f64::consts::PI * 2.0 {
            squid.tentacle_movement -= TAU;
            if entity.random.next_int_bounded(10) == 0 {
                squid.tentacle_speed = 1.0 / (entity.random.next_float() + 1.0) * 0.2;
            }
            anim_synch = true;
        }
        if in_water {
            if squid.tentacle_movement < PI {
                let tentacle_scale = squid.tentacle_movement / PI;
                squid.tentacle_angle = mth::sin((tentacle_scale * tentacle_scale * PI) as f64) * PI * 0.25;
                if tentacle_scale as f64 > 0.75 {
                    if authoritative {
                        entity.set_delta_movement(squid.movement_vector);
                    }
                    squid.rotate_speed = 1.0;
                } else {
                    squid.rotate_speed *= 0.8;
                }
            } else {
                squid.tentacle_angle = 0.0;
                if authoritative {
                    entity.set_delta_movement(entity.delta_movement.scale(0.9));
                }
                squid.rotate_speed *= 0.99;
            }
            let movement = entity.delta_movement;
            let horizontal_movement = movement.horizontal_distance();
            living.y_body_rot += (-(mth::atan2(movement.x, movement.z) as f32) * (180.0 / PI) - living.y_body_rot) * 0.1;
            entity.set_y_rot(living.y_body_rot);
            squid.z_body_rot += PI * squid.rotate_speed * 1.5;
            squid.x_body_rot += (-(mth::atan2(horizontal_movement, movement.y) as f32) * (180.0 / PI) - squid.x_body_rot) * 0.1;
        } else {
            squid.tentacle_angle = mth::sin(squid.tentacle_movement as f64).abs() * PI * 0.25;
            let mut yd = entity.delta_movement.y;
            match levitation {
                Some(amplifier) => yd = 0.05 * (amplifier + 1) as f64,
                None => yd -= gravity,
            }
            entity.set_delta_movement(Vec3::new(0.0, yd * air_drag as f64, 0.0));
            squid.x_body_rot += (-90.0 - squid.x_body_rot) * 0.02;
        }
        if anim_synch {
            self.broadcast_entity_event(id, entity_event::SQUID_ANIM_SYNCH);
        }
    }

    pub fn squid_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.living_entity_hurt_server(id, source, damage) && self.get_last_hurt_by_mob(id).is_some() {
            self.spawn_ink(id);
            true
        } else {
            false
        }
    }

    fn spawn_ink(&mut self, id: EntityId) {
        let mob = self.mob(id).expect("a squid");
        let squirt_sound = sound_event(mob.get_squirt_sound());
        self.make_sound(id, squirt_sound);
        let mob = self.mob_mut(id).expect("a squid");
        let ink_particle = mob.get_ink_particle();
        let is_baby = mob.is_baby();
        let y_body_rot_o = mob.living.y_body_rot_o;
        let e = &mob.entity;
        let pos = squid(mob).rotate_vector(y_body_rot_o, Vec3::new(0.0, -1.0, 0.0)).add(e.x(), e.y(), e.z());
        let mut ink = Vec::with_capacity(30);
        for _ in 0..30 {
            let random = &mut mob.entity.random;
            let x = random.next_float() as f64 * 0.6 - 0.3;
            let z = random.next_float() as f64 * 0.6 - 0.3;
            let dir = squid(mob).rotate_vector(y_body_rot_o, Vec3::new(x, -1.0, z));
            let ink_pos_offset_scale = if is_baby { 0.1f32 } else { 0.3f32 };
            let dir_offset = dir.scale((ink_pos_offset_scale + mob.entity.random.next_float() * 2.0) as f64);
            ink.push(dir_offset);
        }
        for dir_offset in ink {
            self.send_particles(ink_particle, |_| {}, pos.x, pos.y + 0.5, pos.z, 0, (dir_offset.x, dir_offset.y, dir_offset.z), 0.1f32 as f64);
        }
    }

    pub fn squid_travel(&mut self, id: EntityId, _input: Vec3) {
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
    }
}

#[derive(Default)]
pub struct SquidFleeGoal {
    flee_ticks: i32,
}

impl SquidFleeGoal {
    pub fn can_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let entity = level.get_last_hurt_by_mob(mob);
        match entity {
            Some(entity) if level.entity(mob).is_in_water() => {
                let t = level.entity(entity);
                level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z()) < 100.0
            }
            _ => false,
        }
    }

    pub fn start(&mut self) {
        self.flee_ticks = 0;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.flee_ticks += 1;
        let Some(last_hurt_by_mob) = level.get_last_hurt_by_mob(mob) else { return };
        let (s, l) = (level.entity(mob), level.entity(last_hurt_by_mob));
        let (sx, sy, sz) = (s.x(), s.y(), s.z());
        let mut flee_to = Vec3::new(sx - l.x(), sy - l.y(), sz - l.z());
        let pos = BlockPos::containing(sx + flee_to.x, sy + flee_to.y, sz + flee_to.z);
        let block_state = level.block_state(pos);
        let fluid_state = level.fluid_state(pos);
        if fluid_state.is(&fluid_tags::WATER) || block_state.is_air() {
            let length = flee_to.length();
            if length > 0.0 {
                let mut avoid_speed = 3.0;
                if length > 5.0 {
                    avoid_speed -= (length - 5.0) / 5.0;
                }
                if avoid_speed > 0.0 {
                    flee_to = flee_to.scale(avoid_speed);
                }
            }
            if block_state.is_air() {
                flee_to = flee_to.subtract(Vec3::new(0.0, flee_to.y, 0.0));
            }
            squid_mut(level.mob_mut(mob).expect("a squid")).movement_vector = Vec3::new(flee_to.x / 20.0, flee_to.y / 20.0, flee_to.z / 20.0);
        }
    }
}

pub struct SquidRandomMovementGoal;

impl SquidRandomMovementGoal {
    pub fn can_use(&self) -> bool {
        true
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let mob = level.mob_mut(mob).expect("a squid");
        let no_action_time = mob.living.no_action_time;
        if no_action_time > 100 {
            squid_mut(mob).movement_vector = Vec3::ZERO;
        } else if mob.entity.random.next_int_bounded(reduced_tick_delay(50)) == 0 || !mob.entity.was_touching_water || !squid(mob).has_movement_vector() {
            let random = &mut mob.entity.random;
            let angle = random.next_float() * TAU;
            let y = -0.1f32 + random.next_float() * 0.2;
            squid_mut(mob).movement_vector = Vec3::new((mth::cos(angle as f64) * 0.2) as f64, y as f64, (mth::sin(angle as f64) * 0.2) as f64);
        }
    }
}
