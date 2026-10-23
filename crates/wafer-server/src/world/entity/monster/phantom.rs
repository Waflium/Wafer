use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, SoundSource};
use crate::server::level::events;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::{Attribute, PHANTOM_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::tag_value_input;

pub const ID_SIZE: usize = 16;
const XP_REWARD: i32 = 5;
pub const TICKS_PER_FLAP: i32 = 25;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 0.5)
    .with_eye_height(0.175)
    .with_passenger_attachments(&[Vec3::new(0.0, 0.3375f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.125f32 as f64, 0.0)]);

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttackPhase {
    Circle,
    Swoop,
}

pub struct PhantomMoveControl {
    speed: f32,
}

pub struct Phantom {
    move_target_point: Vec3,
    anchor_point: Option<BlockPos>,
    attack_phase: AttackPhase,
    move_control: PhantomMoveControl,
}

impl Default for Phantom {
    fn default() -> Phantom {
        Phantom { move_target_point: Vec3::ZERO, anchor_point: None, attack_phase: AttackPhase::Circle, move_control: PhantomMoveControl { speed: 0.1 } }
    }
}

fn phantom(mob: &Mob) -> &Phantom {
    match &mob.kind {
        MobKind::Phantom(phantom) => phantom,
        _ => unreachable!("a phantom"),
    }
}

fn phantom_mut(mob: &mut Mob) -> &mut Phantom {
    match &mut mob.kind {
        MobKind::Phantom(phantom) => phantom,
        _ => unreachable!("a phantom"),
    }
}

pub fn get_unique_flap_tick_offset(id: EntityId) -> i32 {
    id.wrapping_mul(3)
}

pub fn get_phantom_size(entity: &Entity) -> i32 {
    match entity.data.get(ID_SIZE) {
        DataValue::Int(size) => *size,
        _ => 0,
    }
}

impl Phantom {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:phantom"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&PHANTOM_ATTRIBUTES), MobKind::Phantom(Phantom::default()));
        mob.xp_reward = XP_REWARD;
        mob.ai.move_control.kind = MoveControlKind::Phantom;
        mob.ai.look_control.kind = LookControlKind::Phantom;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::PhantomAttackStrategy(PhantomAttackStrategyGoal::default()));
        goal_selector.add_goal(2, Goal::PhantomSweepAttack(PhantomSweepAttackGoal::default()));
        goal_selector.add_goal(3, Goal::PhantomCircleAroundAnchor(PhantomCircleAroundAnchorGoal::default()));
        ai.target_selector.add_goal(1, Goal::PhantomAttackPlayerTarget(PhantomAttackPlayerTargetGoal::new()));
    }

    pub fn set_phantom_size(mob: &mut Mob, size: i32) {
        let size = size.clamp(0, 64);
        if *mob.entity.data.get(ID_SIZE) != DataValue::Int(size) {
            mob.entity.data.set(ID_SIZE, DataValue::Int(size));
            Phantom::update_phantom_size_info(mob);
        }
    }

    fn update_phantom_size_info(mob: &mut Mob) {
        mob.refresh_dimensions();
        let attack_damage = 6 + get_phantom_size(&mob.entity);
        mob.living.attributes.instance(Attribute::AttackDamage).expect("phantoms have an attack damage").set_base_value(attack_damage as f64);
    }

    pub fn get_default_dimensions(mob: &Mob) -> EntityDimensions {
        DIMENSIONS.scale(1.0 + 0.15 * get_phantom_size(&mob.entity) as f32)
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        phantom_mut(mob).anchor_point = tag_value_input::read(input, "anchor_pos", crate::util::extra_codecs::block_pos);
        let size = tag_value_input::get_i64(input, "size").map_or(0, |v| v as i32);
        Phantom::set_phantom_size(mob, size);
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        if let Some(anchor_point) = phantom(mob).anchor_point {
            output.push(("anchor_pos".into(), Nbt::IntArray(vec![anchor_point.x, anchor_point.y, anchor_point.z])));
        }
        output.push(("size".into(), Nbt::Int(get_phantom_size(&mob.entity))));
    }

    pub fn get_sound_source() -> SoundSource {
        SoundSource::Hostile
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.phantom.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.phantom.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.phantom.death"
    }

    pub fn on_climbable() -> bool {
        false
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    phantom_mut(mob).anchor_point = Some(mob.entity.block_position().offset(0, 5, 0));
    Phantom::set_phantom_size(mob, 0);
    crate::world::entity::mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

fn can_attack(level: &mut ServerLevel, mob: EntityId, target: EntityId, target_conditions: &TargetingConditions) -> bool {
    target_conditions.test(level, Some(mob), target)
}

fn has_attackable_target(level: &mut ServerLevel, mob: EntityId) -> bool {
    match level.get_target(mob) {
        Some(target) => can_attack(level, mob, target, &TargetingConditions::for_combat()),
        None => false,
    }
}

impl ServerLevel {
    pub fn is_phantom(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Phantom(_)))
    }

    pub fn phantom_check_fall_damage(&mut self, _id: EntityId) {}

    pub fn phantom_travel(&mut self, id: EntityId, input: Vec3) {
        self.travel_flying(id, input, 0.2);
    }

    pub fn phantom_body_rotation_control_client_tick(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a phantom");
        mob.entity.y_head_rot = mob.living.y_body_rot;
        mob.living.y_body_rot = mob.entity.y_rot;
    }

    pub fn phantom_move_control_tick(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a phantom");
        if mob.entity.horizontal_collision {
            let y_rot = mob.entity.y_rot + 180.0;
            mob.entity.set_y_rot(y_rot);
            phantom_mut(mob).move_control.speed = 0.1;
        }
        let move_target_point = phantom(mob).move_target_point;
        let e = &mob.entity;
        let mut tdx = move_target_point.x - e.x();
        let tdy = move_target_point.y - e.y();
        let mut tdz = move_target_point.z - e.z();
        let mut sd = (tdx * tdx + tdz * tdz).sqrt();
        if sd.abs() > 1.0E-5f32 as f64 {
            let y_relative_scale = 1.0 - (tdy * 0.7f32 as f64).abs() / sd;
            tdx *= y_relative_scale;
            tdz *= y_relative_scale;
            sd = (tdx * tdx + tdz * tdz).sqrt();
            let sd2 = (tdx * tdx + tdz * tdz + tdy * tdy).sqrt();
            let prev = mob.entity.y_rot;
            let angle = mth::atan2(tdz, tdx) as f32;
            let a = mth::wrap_degrees(mob.entity.y_rot + 90.0);
            let b = mth::wrap_degrees(angle * mth::RAD_TO_DEG);
            mob.entity.set_y_rot(mth::approach_degrees(a, b, 4.0) - 90.0);
            mob.living.y_body_rot = mob.entity.y_rot;
            let y_rot = mob.entity.y_rot;
            let control = &mut phantom_mut(mob).move_control;
            if mth::degrees_difference_abs(prev, y_rot) < 3.0 {
                control.speed = mth::approach(control.speed, 1.8, 0.005 * (1.8 / control.speed));
            } else {
                control.speed = mth::approach(control.speed, 0.2, 0.025);
            }
            let speed = control.speed;
            let x_rot_d = -(mth::atan2(-tdy, sd) * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32;
            mob.entity.set_x_rot(x_rot_d);
            let move_angle = mob.entity.y_rot + 90.0;
            let txd = (speed * mth::cos((move_angle * mth::DEG_TO_RAD) as f64)) as f64 * (tdx / sd2).abs();
            let tzd = (speed * mth::sin((move_angle * mth::DEG_TO_RAD) as f64)) as f64 * (tdz / sd2).abs();
            let tyd = (speed * mth::sin((x_rot_d * mth::DEG_TO_RAD) as f64)) as f64 * (tdy / sd2).abs();
            let movement = mob.entity.delta_movement;
            mob.entity.set_delta_movement(movement.add_vec(Vec3::new(txd, tyd, tzd).subtract(movement).scale(0.2)));
        }
    }

    fn phantom_touching_target(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a phantom");
        phantom(mob).move_target_point.distance_to_sqr(mob.entity.position()) < 4.0
    }

    fn phantom_set_anchor_above_target(&mut self, id: EntityId) {
        if phantom(self.mob(id).expect("a phantom")).anchor_point.is_none() {
            return;
        }
        let Some(target) = self.get_target(id) else { return };
        let target_pos = self.entity(target).block_position();
        let sea_level = self.sea_level();
        let mob = self.mob_mut(id).expect("a phantom");
        let mut anchor_point = target_pos.offset(0, 20 + mob.entity.random.next_int_bounded(20), 0);
        if anchor_point.y < sea_level {
            anchor_point = BlockPos::new(anchor_point.x, sea_level + 1, anchor_point.z);
        }
        phantom_mut(mob).anchor_point = Some(anchor_point);
    }
}

pub struct PhantomAttackPlayerTargetGoal {
    next_scan_tick: i32,
}

impl PhantomAttackPlayerTargetGoal {
    fn new() -> PhantomAttackPlayerTargetGoal {
        PhantomAttackPlayerTargetGoal { next_scan_tick: reduced_tick_delay(20) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.next_scan_tick > 0 {
            self.next_scan_tick -= 1;
            return false;
        }
        self.next_scan_tick = reduced_tick_delay(60);
        let attack_targeting = TargetingConditions::for_combat().range(64.0);
        let bb = level.entity(mob).bounding_box().inflate(16.0, 64.0, 16.0);
        let mut players = level.get_nearby_players(&attack_targeting, mob, &bb);
        if !players.is_empty() {
            players.sort_by(|&a, &b| level.entity(b).y().total_cmp(&level.entity(a).y()));
            for player in players {
                if can_attack(level, mob, player, &TargetingConditions::for_combat()) {
                    level.set_target(mob, Some(player));
                    return true;
                }
            }
        }
        false
    }

    pub fn can_continue_to_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        has_attackable_target(level, mob)
    }
}

#[derive(Default)]
pub struct PhantomAttackStrategyGoal {
    next_sweep_tick: i32,
}

impl PhantomAttackStrategyGoal {
    pub fn can_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        has_attackable_target(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.next_sweep_tick = reduced_tick_delay(10);
        phantom_mut(level.mob_mut(mob).expect("a phantom")).attack_phase = AttackPhase::Circle;
        level.phantom_set_anchor_above_target(mob);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(anchor_point) = phantom(level.mob(mob).expect("a phantom")).anchor_point else { return };
        let height = level.get_heightmap_pos(HeightmapType::MotionBlocking, anchor_point);
        let m = level.mob_mut(mob).expect("a phantom");
        let anchor_point = height.offset(0, 10 + m.entity.random.next_int_bounded(20), 0);
        phantom_mut(m).anchor_point = Some(anchor_point);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if phantom(level.mob(mob).expect("a phantom")).attack_phase != AttackPhase::Circle {
            return;
        }
        self.next_sweep_tick -= 1;
        if self.next_sweep_tick <= 0 {
            phantom_mut(level.mob_mut(mob).expect("a phantom")).attack_phase = AttackPhase::Swoop;
            level.phantom_set_anchor_above_target(mob);
            let random = &mut level.mob_mut(mob).expect("a phantom").entity.random;
            self.next_sweep_tick = reduced_tick_delay((8 + random.next_int_bounded(4)) * 20);
            let pitch = 0.95 + random.next_float() * 0.1;
            level.entity_play_sound(mob, sound_event("minecraft:entity.phantom.swoop"), 10.0, pitch);
        }
    }
}

#[derive(Default)]
pub struct PhantomCircleAroundAnchorGoal {
    angle: f32,
    distance: f32,
    height: f32,
    clockwise: f32,
}

impl PhantomCircleAroundAnchorGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_none() || phantom(level.mob(mob).expect("a phantom")).attack_phase == AttackPhase::Circle
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let random = &mut level.mob_mut(mob).expect("a phantom").entity.random;
        self.distance = 5.0 + random.next_float() * 10.0;
        self.height = -4.0 + random.next_float() * 9.0;
        self.clockwise = if random.next_boolean() { 1.0 } else { -1.0 };
        self.select_next(level, mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let random = &mut level.mob_mut(mob).expect("a phantom").entity.random;
        if random.next_int_bounded(reduced_tick_delay(350)) == 0 {
            self.height = -4.0 + random.next_float() * 9.0;
        }
        if random.next_int_bounded(reduced_tick_delay(250)) == 0 {
            self.distance += 1.0;
            if self.distance > 15.0 {
                self.distance = 5.0;
                self.clockwise = -self.clockwise;
            }
        }
        if random.next_int_bounded(reduced_tick_delay(450)) == 0 {
            self.angle = random.next_float() * 2.0 * std::f32::consts::PI;
            self.select_next(level, mob);
        }
        if level.phantom_touching_target(mob) {
            self.select_next(level, mob);
        }
        let m = level.mob(mob).expect("a phantom");
        let (target_y, y, block_position) = (phantom(m).move_target_point.y, m.entity.y(), m.entity.block_position());
        if target_y < y && !level.block_state(block_position.below()).is_air() {
            self.height = self.height.max(1.0);
            self.select_next(level, mob);
        }
        let m = level.mob(mob).expect("a phantom");
        let (target_y, y) = (phantom(m).move_target_point.y, m.entity.y());
        if target_y > y && !level.block_state(block_position.above()).is_air() {
            self.height = self.height.min(-1.0);
            self.select_next(level, mob);
        }
    }

    fn select_next(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob_mut(mob).expect("a phantom");
        let block_position = m.entity.block_position();
        let phantom = phantom_mut(m);
        let anchor_point = *phantom.anchor_point.get_or_insert(block_position);
        self.angle += self.clockwise * 15.0 * mth::DEG_TO_RAD;
        phantom.move_target_point = Vec3::at_lower_corner_of(anchor_point).add(
            (self.distance * mth::cos(self.angle as f64)) as f64,
            (-4.0 + self.height) as f64,
            (self.distance * mth::sin(self.angle as f64)) as f64,
        );
    }
}

#[derive(Default)]
pub struct PhantomSweepAttackGoal {
    is_scared_of_cat: bool,
    cat_search_tick: i32,
}

const CAT_SEARCH_TICK_DELAY: i32 = 20;

impl PhantomSweepAttackGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some() && phantom(level.mob(mob).expect("a phantom")).attack_phase == AttackPhase::Swoop
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = level.get_target(mob) else { return false };
        if !level.is_alive(target) {
            return false;
        }
        if level.is_player(target) && (level.is_spectator(target) || level.player(target).game_mode.game_type.is_creative()) {
            return false;
        }
        if !self.can_use(level, mob) {
            return false;
        }
        let tick_count = level.entity(mob).tick_count;
        if tick_count > self.cat_search_tick {
            self.cat_search_tick = tick_count + CAT_SEARCH_TICK_DELAY;
            let bb = level.entity(mob).bounding_box().inflate_all(16.0);
            let cats = level.get_living_entities(&bb, |level, entity| level.is_cat(entity) && level.is_alive(entity));
            for &cat in &cats {
                level.cat_hiss(cat);
            }
            self.is_scared_of_cat = !cats.is_empty();
        }
        !self.is_scared_of_cat
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, None);
        phantom_mut(level.mob_mut(mob).expect("a phantom")).attack_phase = AttackPhase::Circle;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        let t = level.entity(target);
        let move_target_point = Vec3::new(t.x(), t.get_y(0.5), t.z());
        let target_bb = t.bounding_box();
        phantom_mut(level.mob_mut(mob).expect("a phantom")).move_target_point = move_target_point;
        if level.entity(mob).bounding_box().inflate_all(0.2f32 as f64).intersects(&target_bb) {
            level.do_hurt_target(mob, target);
            let Some(m) = level.mob_mut(mob) else { return };
            phantom_mut(m).attack_phase = AttackPhase::Circle;
            if !m.entity.is_silent() {
                let pos = m.entity.block_position();
                level.level_event(None, events::SOUND_PHANTOM_BITE, pos, 0);
            }
        } else if level.entity(mob).horizontal_collision || level.living(mob).hurt_time > 0 {
            phantom_mut(level.mob_mut(mob).expect("a phantom")).attack_phase = AttackPhase::Circle;
        }
    }
}
