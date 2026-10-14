use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{damage_types, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::magma_cube::MagmaCube;
use super::slime::Slime;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ageable_mob::{self, AgeableMob};
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation, rotlerp};
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::attributes::{Attribute, AttributeSupplier};
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::conversion_type::ConversionType;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{self, Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, RemovalReason, sound_event};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const ID_SIZE: usize = 18;
pub const MIN_SIZE: i32 = 1;
pub const MAX_SIZE: i32 = 127;

#[derive(Default)]
pub struct AbstractCubeMob {
    pub ageable: AgeableMob,
    pub target_squish: f32,
    pub squish: f32,
    pub o_squish: f32,
    pub was_on_ground: bool,
    pub move_control: CubeMobMoveControl,
}

#[derive(Default)]
pub struct CubeMobMoveControl {
    pub y_rot: f32,
    pub jump_delay: i32,
    pub is_aggressive: bool,
}

impl CubeMobMoveControl {
    pub fn set_direction(&mut self, y_rot: f32, is_aggressive: bool) {
        self.y_rot = y_rot;
        self.is_aggressive = is_aggressive;
    }
}

pub fn abstract_cube_mob_defaults() -> Vec<DataValue> {
    let mut data = ageable_mob_defaults();
    data.push(DataValue::Int(1));
    data
}

pub fn get_size(entity: &Entity) -> i32 {
    match entity.data.get(ID_SIZE) {
        DataValue::Int(size) => *size,
        _ => MIN_SIZE,
    }
}

impl AbstractCubeMob {
    pub fn create_with(entity_type: &str, dimensions: EntityDimensions, attributes: &'static AttributeSupplier, kind: MobKind) -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, abstract_cube_mob_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(attributes), kind);
        mob.entity.dimensions = mob.dimensions();
        mob.ai.move_control.kind = MoveControlKind::CubeMob;
        let y_rot = 180.0 * mob.entity.y_rot / std::f32::consts::PI;
        mob.abstract_cube_mob_mut().expect("a cube mob").move_control.y_rot = y_rot;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = CubeMobFloatGoal::new(ai);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::CubeMobFloat(float_goal));
        goal_selector.add_goal(4, Goal::CubeMobRandomDirection(CubeMobRandomDirectionGoal::default()));
        goal_selector.add_goal(5, Goal::CubeMobKeepOnJumping(CubeMobKeepOnJumpingGoal));
    }
}

impl Mob {
    pub fn is_cube_mob(&self) -> bool {
        matches!(self.kind, MobKind::Slime(_) | MobKind::MagmaCube(_))
    }

    pub fn abstract_cube_mob(&self) -> Option<&AbstractCubeMob> {
        match &self.kind {
            MobKind::Slime(slime) => Some(&slime.abstract_cube_mob),
            MobKind::MagmaCube(magma_cube) => Some(&magma_cube.abstract_cube_mob),
            _ => None,
        }
    }

    pub fn abstract_cube_mob_mut(&mut self) -> Option<&mut AbstractCubeMob> {
        match &mut self.kind {
            MobKind::Slime(slime) => Some(&mut slime.abstract_cube_mob),
            MobKind::MagmaCube(magma_cube) => Some(&mut magma_cube.abstract_cube_mob),
            _ => None,
        }
    }

    pub fn get_size(&self) -> i32 {
        get_size(&self.entity)
    }

    pub fn is_tiny(&self) -> bool {
        self.get_size() <= 1
    }

    pub fn set_size(&mut self, size: i32, update_health: bool) -> bool {
        match self.kind {
            MobKind::Slime(_) => self.slime_set_size(size, update_health),
            MobKind::MagmaCube(_) => self.magma_cube_set_size(size, update_health),
            _ => false,
        }
    }

    pub fn abstract_cube_mob_set_size(&mut self, size: i32, update_health: bool) -> bool {
        let actual_size = size.clamp(MIN_SIZE, MAX_SIZE);
        let changed = *self.entity.data.get(ID_SIZE) != DataValue::Int(actual_size);
        self.entity.data.set(ID_SIZE, DataValue::Int(actual_size));
        if changed {
            self.abstract_cube_mob_on_synced_data_updated();
        }
        self.entity.reapply_position();
        self.refresh_dimensions();
        self.set_cube_mob_health(actual_size);
        let movement_speed = 0.2f32 + 0.1f32 * actual_size as f32;
        self.living.attributes.instance(Attribute::MovementSpeed).expect("cube mobs have a movement speed").set_base_value(movement_speed as f64);
        if update_health {
            let max_health = self.living.attributes.value(Attribute::MaxHealth) as f32;
            self.set_health(max_health);
        }
        changed
    }

    fn set_cube_mob_health(&mut self, actual_size: i32) {
        self.living.attributes.instance(Attribute::MaxHealth).expect("cube mobs have a max health").set_base_value((actual_size * actual_size) as f64);
    }

    fn abstract_cube_mob_on_synced_data_updated(&mut self) {
        self.refresh_dimensions();
        let y_head_rot = self.entity.y_head_rot;
        self.entity.set_y_rot(y_head_rot);
        self.living.y_body_rot = y_head_rot;
    }

    pub fn abstract_cube_mob_add_additional_save_data(&self, out: &mut Vec<(String, Nbt)>) {
        out.push(("Size".into(), Nbt::Int(self.get_size() - 1)));
        let was_on_ground = self.abstract_cube_mob().expect("a cube mob").was_on_ground;
        out.push(("wasOnGround".into(), Nbt::Byte(was_on_ground as i8)));
    }

    pub fn abstract_cube_mob_read_size(&mut self, nbt: &Nbt) {
        let size = nbt.get("Size").and_then(crate::world::level::block::entity::int_value).unwrap_or(0);
        self.set_size(size + 1, false);
    }

    pub fn abstract_cube_mob_read_was_on_ground(&mut self, nbt: &Nbt) {
        let was_on_ground = nbt.get("wasOnGround").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        self.abstract_cube_mob_mut().expect("a cube mob").was_on_ground = was_on_ground;
    }

    pub fn get_particle_type(&self) -> Option<&'static str> {
        match self.kind {
            MobKind::Slime(_) => Slime::get_particle_type(),
            MobKind::MagmaCube(_) => MagmaCube::get_particle_type(),
            _ => None,
        }
    }

    pub fn get_squish_sound(&self) -> &'static str {
        match self.kind {
            MobKind::MagmaCube(_) => MagmaCube::get_squish_sound(self),
            _ => Slime::get_squish_sound(self),
        }
    }

    pub fn get_jump_sound(&self) -> &'static str {
        match self.kind {
            MobKind::MagmaCube(_) => MagmaCube::get_jump_sound(),
            _ => Slime::get_jump_sound(self),
        }
    }

    pub fn abstract_cube_mob_get_sound_volume(&self) -> f32 {
        0.4 * self.get_size() as f32
    }

    pub fn do_play_jump_sound(&self) -> bool {
        self.get_size() > 0
    }

    pub fn get_sound_pitch(&mut self) -> f32 {
        let pitch_adjuster = if self.is_tiny() { 1.4 } else { 0.8 };
        let random = &mut self.entity.random;
        let a = random.next_float();
        let b = random.next_float();
        ((a - b) * 0.2 + 1.0) * pitch_adjuster
    }

    pub fn get_jump_delay(&mut self) -> i32 {
        match self.kind {
            MobKind::MagmaCube(_) => self.magma_cube_get_jump_delay(),
            _ => self.abstract_cube_mob_get_jump_delay(),
        }
    }

    pub fn abstract_cube_mob_get_jump_delay(&mut self) -> i32 {
        self.entity.random.next_int_bounded(20) + 10
    }

    pub fn decrease_squish(&mut self) {
        let factor = if matches!(self.kind, MobKind::MagmaCube(_)) { 0.9 } else { 0.6 };
        self.abstract_cube_mob_mut().expect("a cube mob").target_squish *= factor;
    }

    pub fn abstract_cube_mob_get_passenger_attachment_point(&self) -> Vec3 {
        let dimensions = self.dimensions();
        let scale = self.living.attributes.value(Attribute::Scale) as f32;
        Vec3::new(0.0, dimensions.height as f64 - 0.015625 * self.get_size() as f64 * scale as f64, 0.0)
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let group_data = ageable_mob::finalize_spawn(level, mob, group_data);
    let data = mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data);
    set_spawn_size(level, mob, difficulty);
    data
}

fn set_spawn_size(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, difficulty: &DifficultyInstance) {
    let random = level.get_random();
    let mut size_scale = random.next_int_bounded(3);
    if size_scale < 2 && random.next_float() < 0.5 * difficulty.get_special_multiplier() {
        size_scale += 1;
    }
    let size = 1 << size_scale;
    mob.set_size(size, true);
}

impl ServerLevel {
    pub fn set_size(&mut self, id: EntityId, size: i32, update_health: bool) {
        let changed = self.mob_mut(id).expect("a cube mob").set_size(size, update_health);
        if changed && self.entity(id).is_in_water() && self.entity_mut(id).random.next_int_bounded(20) == 0 {
            self.do_water_splash_effect(id);
        }
    }

    pub fn abstract_cube_mob_tick(&mut self, id: EntityId) {
        let cube_mob = self.mob_mut(id).expect("a cube mob").abstract_cube_mob_mut().expect("a cube mob");
        cube_mob.o_squish = cube_mob.squish;
        cube_mob.squish += (cube_mob.target_squish - cube_mob.squish) * 0.5;
        self.mob_tick(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let on_ground = mob.entity.on_ground;
        let was_on_ground = mob.abstract_cube_mob().expect("a cube mob").was_on_ground;
        if on_ground && !was_on_ground {
            let size = mob.dimensions().width * 2.0;
            let mut i = 0;
            while (i as f32) < size * 16.0 {
                let random = &mut mob.entity.random;
                random.next_float();
                random.next_float();
                i += 1;
            }
            let squish_sound = mob.get_squish_sound();
            let volume = mob.abstract_cube_mob_get_sound_volume();
            let random = &mut mob.entity.random;
            let a = random.next_float();
            let b = random.next_float();
            let pitch = ((a - b) * 0.2 + 1.0) / 0.8;
            self.entity_play_sound(id, sound_event(squish_sound), volume, pitch);
            self.mob_mut(id).expect("a cube mob").abstract_cube_mob_mut().expect("a cube mob").target_squish = -0.5;
        } else if !on_ground && was_on_ground {
            mob.abstract_cube_mob_mut().expect("a cube mob").target_squish = 1.0;
        }
        let mob = self.mob_mut(id).expect("a cube mob");
        mob.abstract_cube_mob_mut().expect("a cube mob").was_on_ground = on_ground;
        mob.decrease_squish();
    }

    pub fn abstract_cube_mob_remove(&mut self, id: EntityId, reason: RemovalReason) {
        let mob = self.mob(id).expect("a cube mob");
        let size = mob.get_size();
        if size > 1 && self.is_dead_or_dying(id) {
            let width = mob.dimensions().width;
            let entity_type = registry::ENTITY_TYPE[mob.entity.entity_type as usize];
            let xz_cube_spawn_offset = width / 2.0;
            let half_size = size / 2;
            let count = self.get_split_count(id);
            let team = self.team_of(id).map(|team| team.name().to_string());
            for i in 0..count {
                let xd = ((i % 2) as f32 - 0.5) * xz_cube_spawn_offset;
                let zd = ((i / 2) as f32 - 0.5) * xz_cube_spawn_offset;
                let params = ConversionParams { ty: ConversionType::SplitOnDeath, keep_equipment: false, preserve_can_pick_up_loot: false, team: team.clone() };
                self.convert_to(id, entity_type, params, |level, cube_mob| level.set_up_split_cube(id, cube_mob, half_size, xd, zd));
            }
        }
        self.living_entity_remove(id, reason);
    }

    fn set_up_split_cube(&mut self, id: EntityId, cube_mob: EntityId, half_size: i32, xd: f32, zd: f32) {
        self.set_size(cube_mob, half_size, true);
        let e = self.entity_mut(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let y_rot = e.random.next_float() * 360.0;
        self.entity_mut(cube_mob).snap_to(x + xd as f64, y + 0.5, z + zd as f64, y_rot, 0.0);
    }

    fn get_split_count(&mut self, id: EntityId) -> i32 {
        2 + self.entity_mut(id).random.next_int_bounded(3)
    }

    pub fn abstract_cube_mob_push(&mut self, id: EntityId, entity: EntityId) {
        self.living_entity_push(id, entity);
        if registry::ENTITY_TYPE[self.entity(entity).entity_type as usize] == "minecraft:iron_golem" && self.can_deal_damage(id) {
            self.deal_damage(id, entity);
        }
    }

    pub fn abstract_cube_mob_player_touch(&mut self, id: EntityId, player: EntityId) {
        if self.can_deal_damage(id) {
            self.deal_damage(id, player);
        }
    }

    fn deal_damage(&mut self, id: EntityId, target: EntityId) {
        if self.is_alive(id) && self.do_teams_allow_damage(id, target) && self.is_within_melee_attack_range(id, target) && self.has_line_of_sight(id, target) {
            let damage_source = DamageSource { causing_entity: Some(id), direct_entity: Some(id), ..DamageSource::new(damage_types::MOB_ATTACK) };
            let damage = self.get_attack_damage(id);
            if self.hurt_server(target, damage_source, damage) {
                let random = &mut self.entity_mut(id).random;
                let a = random.next_float();
                let b = random.next_float();
                self.entity_play_sound(id, sound_event("minecraft:entity.slime.attack"), 1.0, (a - b) * 0.2 + 1.0);
                self.do_post_attack_effects(target, &damage_source);
            }
        }
    }

    pub fn can_deal_damage(&self, id: EntityId) -> bool {
        match self.mob(id).expect("a cube mob").kind {
            MobKind::MagmaCube(_) => self.magma_cube_can_deal_damage(id),
            _ => self.abstract_cube_mob_can_deal_damage(id),
        }
    }

    pub fn abstract_cube_mob_can_deal_damage(&self, id: EntityId) -> bool {
        !self.mob(id).expect("a cube mob").is_tiny() && self.is_effective_ai(id)
    }

    fn get_attack_damage(&self, id: EntityId) -> f32 {
        match self.mob(id).expect("a cube mob").kind {
            MobKind::MagmaCube(_) => self.magma_cube_get_attack_damage(id),
            _ => self.abstract_cube_mob_get_attack_damage(id),
        }
    }

    pub fn abstract_cube_mob_get_attack_damage(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::AttackDamage) as f32
    }

    pub fn abstract_cube_mob_jump_from_ground(&mut self, id: EntityId) {
        let jump_power = self.get_jump_power(id);
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        e.set_delta_movement(Vec3::new(movement.x, jump_power as f64, movement.z));
        e.needs_sync = true;
    }

    pub fn abstract_cube_mob_get_sound_source(&self) -> SoundSource {
        SoundSource::Hostile
    }

    pub fn cube_mob_move_control_tick(&mut self, id: EntityId) {
        let movement_speed = self.attribute_value(id, Attribute::MovementSpeed);
        let mob = self.mob_mut(id).expect("a cube mob");
        let wanted_y_rot = mob.abstract_cube_mob().expect("a cube mob").move_control.y_rot;
        let y_rot = rotlerp(mob.entity.y_rot, wanted_y_rot, 90.0);
        mob.entity.set_y_rot(y_rot);
        mob.entity.y_head_rot = mob.entity.y_rot;
        mob.living.y_body_rot = mob.entity.y_rot;
        if mob.ai.move_control.operation != Operation::MoveTo {
            mob.living.zza = 0.0;
            return;
        }
        mob.ai.move_control.operation = Operation::Wait;
        let speed = (mob.ai.move_control.speed_modifier * movement_speed) as f32;
        if !mob.entity.on_ground {
            mob.set_speed(speed);
            return;
        }
        mob.set_speed(speed);
        let move_control = &mut mob.abstract_cube_mob_mut().expect("a cube mob").move_control;
        let jump_delay = move_control.jump_delay;
        move_control.jump_delay -= 1;
        if jump_delay <= 0 {
            let mut jump_delay = mob.get_jump_delay();
            let move_control = &mut mob.abstract_cube_mob_mut().expect("a cube mob").move_control;
            if move_control.is_aggressive {
                jump_delay /= 3;
            }
            move_control.jump_delay = jump_delay;
            mob.ai.jump_control.jump();
            if mob.do_play_jump_sound() {
                let jump_sound = mob.get_jump_sound();
                let volume = mob.abstract_cube_mob_get_sound_volume();
                let pitch = mob.get_sound_pitch();
                self.entity_play_sound(id, sound_event(jump_sound), volume, pitch);
            }
        } else {
            mob.living.xxa = 0.0;
            mob.living.zza = 0.0;
            mob.set_speed(0.0);
        }
    }

    fn set_wanted_movement(&mut self, id: EntityId, speed_modifier: f64) {
        let move_control = &mut self.mob_mut(id).expect("a cube mob").ai.move_control;
        move_control.speed_modifier = speed_modifier;
        move_control.operation = Operation::MoveTo;
    }

    fn set_direction(&mut self, id: EntityId, y_rot: f32, is_aggressive: bool) {
        self.mob_mut(id).expect("a cube mob").abstract_cube_mob_mut().expect("a cube mob").move_control.set_direction(y_rot, is_aggressive);
    }
}

#[derive(Default)]
pub struct CubeMobAttackGoal {
    grow_tired_timer: i32,
}

impl CubeMobAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match level.get_target(mob) {
            None => false,
            Some(target) => level.can_attack(mob, target),
        }
    }

    pub fn start(&mut self) {
        self.grow_tired_timer = reduced_tick_delay(300);
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match level.get_target(mob) {
            None => false,
            Some(target) if !level.can_attack(mob, target) => false,
            Some(_) => {
                self.grow_tired_timer -= 1;
                self.grow_tired_timer > 0
            }
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(target) = level.get_target(mob) {
            level.look_at(mob, target, 10.0, 10.0);
        }
        let y_rot = level.entity(mob).y_rot;
        let is_aggressive = level.can_deal_damage(mob);
        level.set_direction(mob, y_rot, is_aggressive);
    }
}

pub struct CubeMobFloatGoal;

impl CubeMobFloatGoal {
    pub fn new(ai: &mut MobAi) -> CubeMobFloatGoal {
        ai.navigation.set_can_float(true);
        CubeMobFloatGoal
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.entity(mob).is_in_liquid()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob_mut(mob).expect("a cube mob");
        if m.entity.random.next_float() < 0.8 {
            m.ai.jump_control.jump();
        }
        level.set_wanted_movement(mob, 1.2);
    }
}

pub struct CubeMobKeepOnJumpingGoal;

impl CubeMobKeepOnJumpingGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_passenger(mob)
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_wanted_movement(mob, 1.0);
    }
}

#[derive(Default)]
pub struct CubeMobRandomDirectionGoal {
    chosen_degrees: f32,
    next_randomize_time: i32,
}

impl CubeMobRandomDirectionGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        level.get_target(mob).is_none() && (e.on_ground || e.is_in_liquid() || level.has_effect(mob, mob_effects::LEVITATION))
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.next_randomize_time -= 1;
        if self.next_randomize_time <= 0 {
            let random = &mut level.entity_mut(mob).random;
            self.next_randomize_time = reduced_tick_delay(40 + random.next_int_bounded(60));
            self.chosen_degrees = random.next_int_bounded(360) as f32;
        }
        level.set_direction(mob, self.chosen_degrees, false);
    }
}
