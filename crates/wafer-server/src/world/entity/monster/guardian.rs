use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{damage_type_tags, damage_types, fluid_tags, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation, rotlerp};
use crate::world::entity::ai::goal::goal::{Flag, Goal};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::move_towards_restriction_goal::MoveTowardsRestrictionGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::{Attribute, AttributeSupplier};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::pathfinder::path_type::PathType;

pub const DATA_ID_MOVING: usize = 16;
pub const DATA_ID_ATTACK_TARGET: usize = 17;
const XP_REWARD: i32 = 10;
pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.85, 0.85).with_eye_height(0.425).with_passenger_attachments(&[Vec3::new(0.0, 0.975f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Guardian {
    random_stroll_trigger: bool,
}

fn guardian_mut(mob: &mut Mob) -> &mut Guardian {
    match &mut mob.kind {
        MobKind::Guardian(guardian) | MobKind::ElderGuardian(guardian) => guardian,
        _ => unreachable!("a guardian"),
    }
}

fn is_elder(mob: &Mob) -> bool {
    matches!(mob.kind, MobKind::ElderGuardian(_))
}

pub fn is_moving(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_ID_MOVING), DataValue::Boolean(true))
}

fn set_moving(entity: &mut Entity, value: bool) {
    entity.data.set(DATA_ID_MOVING, DataValue::Boolean(value));
}

fn set_active_attack_target(entity: &mut Entity, entity_id: i32) {
    entity.data.set(DATA_ID_ATTACK_TARGET, DataValue::Int(entity_id));
}

pub fn has_active_attack_target(entity: &Entity) -> bool {
    !matches!(entity.data.get(DATA_ID_ATTACK_TARGET), DataValue::Int(0))
}

fn guardian_attack_selector(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    (level.is_player(target) || level.mob(target).is_some_and(Mob::is_squid) || TargetType::Axolotl.is_instance(level, target))
        && level.entity(target).distance_to_sqr(level.entity(mob)) > 9.0
}

impl Guardian {
    pub fn create() -> Mob {
        Guardian::create_kind("minecraft:guardian", DIMENSIONS, &crate::world::entity::attributes::GUARDIAN_ATTRIBUTES, MobKind::Guardian(Guardian::default()))
    }

    pub fn create_kind(entity_type: &str, dimensions: EntityDimensions, attributes: &'static AttributeSupplier, kind: MobKind) -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, data);
        let mut mob = Mob::new(entity, LivingEntity::new(attributes), kind);
        mob.xp_reward = XP_REWARD;
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        mob.ai.move_control.kind = MoveControlKind::Guardian;
        mob.entity.random.next_float();
        mob
    }

    pub fn register_goals(ai: &mut MobAi, random_stroll_interval: i32) {
        let mut goal = MoveTowardsRestrictionGoal::new(1.0);
        let mut random_stroll_goal = RandomStrollGoal::with_interval(1.0, random_stroll_interval);
        random_stroll_goal.set_flags(Flag::Move.bit() | Flag::Look.bit());
        goal.set_flags(Flag::Move.bit() | Flag::Look.bit());
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(4, Goal::GuardianAttack(GuardianAttackGoal::default()));
        goal_selector.add_goal(5, Goal::MoveTowardsRestriction(goal));
        goal_selector.add_goal(7, Goal::RandomStroll(random_stroll_goal));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Guardian, 12.0).with_probability(0.01)));
        goal_selector.add_goal(9, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        ai.target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::LivingEntity, 10, true, false, Some(guardian_attack_selector))));
    }

    pub fn get_attack_duration(mob: &Mob) -> i32 {
        if is_elder(mob) { 60 } else { 80 }
    }

    pub fn get_ambient_sound_interval() -> i32 {
        160
    }

    pub fn get_ambient_sound(mob: &Mob) -> &'static str {
        match (is_elder(mob), mob.entity.is_in_water()) {
            (false, true) => "minecraft:entity.guardian.ambient",
            (false, false) => "minecraft:entity.guardian.ambient_land",
            (true, true) => "minecraft:entity.elder_guardian.ambient",
            (true, false) => "minecraft:entity.elder_guardian.ambient_land",
        }
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        match (is_elder(mob), mob.entity.is_in_water()) {
            (false, true) => "minecraft:entity.guardian.hurt",
            (false, false) => "minecraft:entity.guardian.hurt_land",
            (true, true) => "minecraft:entity.elder_guardian.hurt",
            (true, false) => "minecraft:entity.elder_guardian.hurt_land",
        }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        match (is_elder(mob), mob.entity.is_in_water()) {
            (false, true) => "minecraft:entity.guardian.death",
            (false, false) => "minecraft:entity.guardian.death_land",
            (true, true) => "minecraft:entity.elder_guardian.death",
            (true, false) => "minecraft:entity.elder_guardian.death_land",
        }
    }

    pub fn get_max_head_x_rot() -> i32 {
        180
    }
}

pub fn get_walk_target_value(pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
    if level.fluid_state(pos).is(&fluid_tags::WATER) {
        10.0 + level.get_pathfinding_cost_from_light_levels(pos)
    } else {
        super::monster::get_walk_target_value(pos, level)
    }
}

impl ServerLevel {
    pub fn is_guardian(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Guardian(_) | MobKind::ElderGuardian(_)))
    }

    pub fn guardian_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        if PathNavigationRegion::new(self).get_fluid_state(pos).is(&fluid_tags::WATER) {
            10.0 + self.get_pathfinding_cost_from_light_levels(pos)
        } else {
            self.monster_get_walk_target_value(pos)
        }
    }

    pub fn guardian_take_random_stroll_trigger(&mut self, id: EntityId) -> bool {
        match self.mob_mut(id).map(|mob| &mut mob.kind) {
            Some(MobKind::Guardian(guardian) | MobKind::ElderGuardian(guardian)) => std::mem::take(&mut guardian.random_stroll_trigger),
            _ => false,
        }
    }

    fn guardian_trigger_random_stroll(&mut self, id: EntityId) {
        guardian_mut(self.mob_mut(id).expect("a guardian")).random_stroll_trigger = true;
    }

    pub fn guardian_ai_step(&mut self, id: EntityId) {
        if self.is_alive(id) {
            let e = self.entity(id);
            if e.is_in_water() {
                self.entity_mut(id).set_air_supply(300);
            } else if e.on_ground {
                let e = self.entity_mut(id);
                let xa = ((e.random.next_float() * 2.0 - 1.0) * 0.4) as f64;
                let za = ((e.random.next_float() * 2.0 - 1.0) * 0.4) as f64;
                e.set_delta_movement(e.delta_movement.add(xa, 0.5, za));
                let y_rot = e.random.next_float() * 360.0;
                e.set_y_rot(y_rot);
                self.set_on_ground(id, false);
                self.entity_mut(id).needs_sync = true;
            }
            let e = self.entity_mut(id);
            if has_active_attack_target(e) {
                let y_head_rot = e.y_head_rot;
                e.set_y_rot(y_head_rot);
            }
        }
        self.monster_ai_step(id);
    }

    pub fn guardian_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if !is_moving(self.entity(id))
            && !source.is(&damage_type_tags::AVOIDS_GUARDIAN_THORNS)
            && !source.is_type(damage_types::THORNS)
            && let Some(cause) = source.direct_entity.filter(|&direct| self.is_living(direct))
        {
            self.hurt_server(cause, DamageSource::entity(damage_types::THORNS, id), 2.0);
        }
        self.guardian_trigger_random_stroll(id);
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn guardian_travel_in_water(&mut self, id: EntityId, input: Vec3) {
        self.entity_mut(id).move_relative(0.1, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, crate::world::entity::MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.9));
        if !is_moving(self.entity(id)) && self.get_target(id).is_none() {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -0.005, 0.0));
        }
    }

    pub fn guardian_move_control_tick(&mut self, id: EntityId) {
        let movement_speed = self.attribute_value(id, Attribute::MovementSpeed);
        let speed = self.get_speed(id);
        let mob = self.mob_mut(id).expect("a guardian");
        let control = &mob.ai.move_control;
        if control.operation == Operation::MoveTo && !mob.ai.navigation.is_done() {
            let e = &mut mob.entity;
            let delta = Vec3::new(control.wanted_x - e.x(), control.wanted_y - e.y(), control.wanted_z - e.z());
            let length = delta.length();
            let xd = delta.x / length;
            let yd = delta.y / length;
            let zd = delta.z / length;
            let y_rot_d = (mth::atan2(delta.z, delta.x) * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32 - 90.0;
            e.set_y_rot(rotlerp(e.y_rot, y_rot_d, 90.0));
            mob.living.y_body_rot = e.y_rot;
            let target_speed = (control.speed_modifier * movement_speed) as f32;
            let new_speed = mth::lerp_f(0.125, speed, target_speed);
            let tick = (e.tick_count + e.id as i32) as f64;
            let push = (tick * 0.5).sin() * 0.05;
            let cos = ((e.y_rot * mth::DEG_TO_RAD) as f64).cos();
            let sin = ((e.y_rot * mth::DEG_TO_RAD) as f64).sin();
            let y_push = (tick * 0.75).sin() * 0.05;
            e.set_delta_movement(e.delta_movement.add(push * cos, y_push * (sin + cos) * 0.25 + new_speed as f64 * yd * 0.1, push * sin));
            let new_look_x = e.x() + xd * 2.0;
            let new_look_y = e.eye_y() + yd / length;
            let new_look_z = e.z() + zd * 2.0;
            let look = &mut mob.ai.look_control;
            let (mut old_look_x, mut old_look_y, mut old_look_z) = (look.wanted_x, look.wanted_y, look.wanted_z);
            if !look.is_looking_at_target() {
                old_look_x = new_look_x;
                old_look_y = new_look_y;
                old_look_z = new_look_z;
            }
            look.set_look_at(mth::lerp(0.125, old_look_x, new_look_x), mth::lerp(0.125, old_look_y, new_look_y), mth::lerp(0.125, old_look_z, new_look_z), 10.0, 40.0);
            set_moving(&mut mob.entity, true);
            self.set_speed(id, new_speed);
        } else {
            set_moving(&mut mob.entity, false);
            self.set_speed(id, 0.0);
        }
    }
}

#[derive(Default)]
pub struct GuardianAttackGoal {
    attack_time: i32,
}

impl GuardianAttackGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some_and(|target| level.is_alive(target))
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.can_use(level, mob) && (is_elder(level.mob(mob).expect("a guardian")) || level.get_target(mob).is_some_and(|target| level.entity(mob).distance_to_sqr(level.entity(target)) > 9.0))
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.attack_time = -10;
        level.get_navigation(mob, |navigation, _| navigation.stop());
        if let Some(target) = level.get_target(mob) {
            level.set_look_at_entity(mob, target, 90.0, 90.0);
        }
        level.entity_mut(mob).needs_sync = true;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_active_attack_target(level.entity_mut(mob), 0);
        level.set_target(mob, None);
        level.guardian_trigger_random_stroll(mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        level.get_navigation(mob, |navigation, _| navigation.stop());
        level.set_look_at_entity(mob, target, 90.0, 90.0);
        if !level.has_line_of_sight(mob, target) {
            level.set_target(mob, None);
            return;
        }
        self.attack_time += 1;
        let m = level.mob(mob).expect("a guardian");
        if self.attack_time == 0 {
            let target_id = level.entity(target).id as i32;
            set_active_attack_target(level.entity_mut(mob), target_id);
            if !level.entity(mob).is_silent() {
                level.broadcast_entity_event(mob, 21);
            }
        } else if self.attack_time >= Guardian::get_attack_duration(m) {
            let elder = is_elder(m);
            let mut magic_damage = 1.0;
            if level.difficulty == Difficulty::Hard {
                magic_damage += 2.0;
            }
            if elder {
                magic_damage += 2.0;
            }
            level.hurt_server(target, DamageSource::source(damage_types::INDIRECT_MAGIC, Some(mob), Some(mob)), magic_damage);
            level.do_hurt_target(mob, target);
            level.set_target(mob, None);
        }
    }
}
