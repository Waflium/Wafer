use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Direction, Vec3, mth};
use wafer_data::{block_tags, damage_type_tags, fluid_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::events::{SOUND_GHAST_FIREBALL, SOUND_GHAST_WARNING};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation};
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::{Attribute, GHAST_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::hurtingprojectile::large_fireball::LargeFireball;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::clip::for_each_block_intersected_between;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DATA_IS_CHARGING: usize = 16;
pub const DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(4.0, 4.0).with_eye_height(2.6).with_passenger_attachments(&[Vec3::new(0.0, 4.0625f32 as f64, 0.0)]).with_vehicle_attachment(&[Vec3::new(0.0, -0.5f32 as f64, 0.0)]);
const DEFAULT_EXPLOSION_POWER: i8 = 1;
const XP_REWARD: i32 = 5;

#[derive(Clone, Copy, PartialEq)]
pub struct GhastMoveControl {
    float_duration: i32,
    careful: bool,
}

impl GhastMoveControl {
    pub const fn new(careful: bool) -> GhastMoveControl {
        GhastMoveControl { float_duration: 0, careful }
    }
}

pub struct Ghast {
    explosion_power: i32,
}

impl Default for Ghast {
    fn default() -> Ghast {
        Ghast { explosion_power: DEFAULT_EXPLOSION_POWER as i32 }
    }
}

fn ghast(mob: &Mob) -> &Ghast {
    match &mob.kind {
        MobKind::Ghast(ghast) => ghast,
        _ => unreachable!("not a ghast"),
    }
}

fn ghast_mut(mob: &mut Mob) -> &mut Ghast {
    match &mut mob.kind {
        MobKind::Ghast(ghast) => ghast,
        _ => unreachable!("not a ghast"),
    }
}

pub fn ghast_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.push(DataValue::Boolean(false));
    data
}

pub fn is_charging(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IS_CHARGING), DataValue::Boolean(true))
}

fn set_charging(entity: &mut Entity, on_off: bool) {
    entity.data.set(DATA_IS_CHARGING, DataValue::Boolean(on_off));
}

fn target_within_four_blocks(mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    (level.entity(target).y() - level.entity(mob).y()).abs() <= 4.0
}

impl Ghast {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:ghast"), DIMENSIONS, ghast_defaults());
        let mut mob = Mob::new(entity, LivingEntity::new(&GHAST_ATTRIBUTES), MobKind::Ghast(Ghast::default()));
        mob.xp_reward = XP_REWARD;
        mob.ai.move_control.kind = MoveControlKind::Ghast(GhastMoveControl::new(false));
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(5, Goal::RandomFloatAround(RandomFloatAroundGoal::new(0)));
        goal_selector.add_goal(7, Goal::GhastLook);
        goal_selector.add_goal(7, Goal::GhastShootFireball(GhastShootFireballGoal::default()));
        ai.target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(target_within_four_blocks))));
    }

    pub fn get_explosion_power(mob: &Mob) -> i32 {
        ghast(mob).explosion_power
    }

    pub fn get_sound_source() -> SoundSource {
        SoundSource::Hostile
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.ghast.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.ghast.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.ghast.death"
    }

    pub fn get_sound_volume() -> f32 {
        5.0
    }

    pub fn get_max_spawn_cluster_size() -> i32 {
        1
    }

    pub fn on_climbable() -> bool {
        false
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("ExplosionPower".into(), Nbt::Byte(ghast(mob).explosion_power as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let explosion_power = match input.get("ExplosionPower") {
            Some(Nbt::Byte(value)) => *value,
            Some(value) => value.as_i64().map_or(DEFAULT_EXPLOSION_POWER, |value| value as i8),
            None => DEFAULT_EXPLOSION_POWER,
        };
        ghast_mut(mob).explosion_power = explosion_power as i32;
    }
}

fn is_reflected_fireball(level: &ServerLevel, source: &DamageSource) -> bool {
    source.direct_entity.and_then(|direct| level.as_projectile(direct)).is_some_and(|projectile| matches!(projectile.kind, ProjectileKind::LargeFireball(_)))
        && source.causing_entity.is_some_and(|causing| level.is_player(causing))
}

pub fn face_movement_direction(level: &mut ServerLevel, ghast: EntityId) {
    match level.get_target(ghast) {
        None => {
            let mob = level.mob_mut(ghast).expect("a ghast");
            let movement = mob.entity.delta_movement;
            mob.entity.set_y_rot(-(mth::atan2(movement.x, movement.z) as f32) * mth::RAD_TO_DEG);
            mob.living.y_body_rot = mob.entity.y_rot;
        }
        Some(target) => {
            let (t, g) = (level.entity(target), level.entity(ghast));
            if t.distance_to_sqr(g) < 4096.0 {
                let xdd = t.x() - g.x();
                let zdd = t.z() - g.z();
                let mob = level.mob_mut(ghast).expect("a ghast");
                mob.entity.set_y_rot(-(mth::atan2(xdd, zdd) as f32) * mth::RAD_TO_DEG);
                mob.living.y_body_rot = mob.entity.y_rot;
            }
        }
    }
}

impl ServerLevel {
    pub fn ghast_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        self.entity(id).is_invulnerable() && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY)
            || !is_reflected_fireball(self, source) && self.living_entity_is_invulnerable_to(id, source)
    }

    pub fn ghast_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if is_reflected_fireball(self, &source) {
            self.living_entity_hurt_server(id, source, 1000.0);
            true
        } else if self.ghast_is_invulnerable_to(id, &source) {
            false
        } else {
            self.living_entity_hurt_server(id, source, damage)
        }
    }

    pub fn ghast_check_fall_damage(&mut self, _id: EntityId) {}

    pub fn ghast_travel(&mut self, id: EntityId, input: Vec3) {
        self.travel_flying(id, input, 0.02);
    }

    fn ghast_move_control_should_be_stopped(&self, id: EntityId) -> bool {
        self.is_happy_ghast(id) && self.happy_ghast_is_on_still_timeout(id)
    }

    fn ghast_move_control_mut(mob: &mut Mob) -> &mut GhastMoveControl {
        match &mut mob.ai.move_control.kind {
            MoveControlKind::Ghast(control) => control,
            _ => unreachable!("a ghast move control"),
        }
    }

    pub fn ghast_move_control_tick(&mut self, id: EntityId) {
        if self.ghast_move_control_should_be_stopped(id) {
            self.mob_mut(id).expect("a mob").ai.move_control.operation = Operation::Wait;
            self.stop_in_place(id);
        }
        let flying_speed = self.attribute_value(id, Attribute::FlyingSpeed);
        let mob = self.mob_mut(id).expect("a ghast");
        if mob.ai.move_control.operation != Operation::MoveTo {
            return;
        }
        let control = ServerLevel::ghast_move_control_mut(mob);
        let float_duration = control.float_duration;
        control.float_duration -= 1;
        if float_duration > 0 {
            return;
        }
        let increase = mob.entity.random.next_int_bounded(5) + 2;
        ServerLevel::ghast_move_control_mut(mob).float_duration += increase;
        let control = &mob.ai.move_control;
        let travel = Vec3::new(control.wanted_x - mob.entity.x(), control.wanted_y - mob.entity.y(), control.wanted_z - mob.entity.z());
        if self.ghast_move_control_can_reach(id, travel) {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add_vec(travel.normalize().scale(flying_speed * 5.0 / 3.0)));
        } else {
            self.mob_mut(id).expect("a ghast").ai.move_control.operation = Operation::Wait;
        }
    }

    fn ghast_move_control_can_reach(&mut self, id: EntityId, travel: Vec3) -> bool {
        let careful = ServerLevel::ghast_move_control_mut(self.mob_mut(id).expect("a ghast")).careful;
        let aabb = self.entity(id).bounding_box();
        let aabb_at_destination = aabb.move_vec(travel);
        if careful {
            let inflated = aabb_at_destination.inflate_all(1.0);
            let from = BlockPos::containing(inflated.min_x, inflated.min_y, inflated.min_z);
            let to = BlockPos::containing(inflated.max_x, inflated.max_y, inflated.max_z);
            for pos in BlockPos::between_closed(from, to) {
                if !self.block_traversal_possible(id, None, pos, false, false) {
                    return false;
                }
            }
        }
        let e = self.entity(id);
        let (is_in_water, is_in_lava) = (e.is_in_water(), e.is_in_lava());
        let start = e.position();
        let end = start.add_vec(travel);
        for_each_block_intersected_between(start, end, &aabb_at_destination, |block_pos, _| {
            aabb.intersects_block(block_pos) || self.block_traversal_possible(id, Some((start, end)), block_pos, is_in_water, is_in_lava)
        })
    }

    fn block_traversal_possible(&mut self, id: EntityId, start_end: Option<(Vec3, Vec3)>, pos: BlockPos, can_path_through_water: bool, can_path_through_lava: bool) -> bool {
        let state = self.block_state(pos);
        if state.is_air() {
            return true;
        }
        let careful = ServerLevel::ghast_move_control_mut(self.mob_mut(id).expect("a ghast")).careful;
        let path_no_collisions = match start_end {
            Some((start, end)) => {
                let boxes: Vec<AABB> = self.get_state_collision_shape(state, pos).to_aabbs().iter().map(|b| b.move_to_block(pos)).collect();
                !self.collided_with_shape_moving_from(id, start, end, &boxes)
            }
            None => self.get_state_collision_shape(state, pos).is_empty(),
        };
        if !careful {
            return path_no_collisions;
        }
        if state.is(&block_tags::HAPPY_GHAST_AVOIDS) {
            return false;
        }
        let fluid_state = state.fluid_state();
        if !fluid_state.is_empty() && start_end.is_none_or(|(start, end)| self.collided_with_fluid(id, fluid_state, pos, start, end)) {
            if fluid_state.is(&fluid_tags::WATER) {
                return can_path_through_water;
            }
            if fluid_state.is(&fluid_tags::LAVA) {
                return can_path_through_lava;
            }
        }
        path_no_collisions
    }

}

pub struct RandomFloatAroundGoal {
    distance_to_blocks: i32,
}

impl RandomFloatAroundGoal {
    pub fn new(distance_to_blocks: i32) -> RandomFloatAroundGoal {
        RandomFloatAroundGoal { distance_to_blocks }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a mob");
        let move_control = &m.ai.move_control;
        if !move_control.has_wanted() {
            return true;
        }
        let xd = move_control.wanted_x - m.entity.x();
        let yd = move_control.wanted_y - m.entity.y();
        let zd = move_control.wanted_z - m.entity.z();
        let dd = xd * xd + yd * yd + zd * zd;
        dd < 1.0 || dd > 3600.0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let result = get_suitable_fly_to_position(level, mob, self.distance_to_blocks);
        level.mob_mut(mob).expect("a mob").ai.move_control.set_wanted_position(result.x, result.y, result.z, 1.0);
    }
}

pub fn get_suitable_fly_to_position(level: &mut ServerLevel, mob: EntityId, distance_to_blocks: i32) -> Vec3 {
    let center = level.entity(mob).position();
    let mut result = None;
    for _ in 0..64 {
        result = choose_random_position_with_restriction(level, mob, center);
        if let Some(target) = result
            && is_good_target(level, target, distance_to_blocks)
        {
            return target;
        }
    }
    let mut result = match result {
        Some(result) => result,
        None => choose_random_position(center, &mut level.entity_mut(mob).random),
    };
    let pos = BlockPos::containing_vec(result);
    let height_y = level.get_height(HeightmapType::MotionBlocking, pos.x, pos.z);
    if height_y < pos.y && height_y > level.get_min_y() {
        let y = level.entity(mob).y();
        result = Vec3::new(result.x, y - (y - result.y).abs(), result.z);
    }
    result
}

fn is_good_target(level: &mut ServerLevel, target: Vec3, distance_to_blocks: i32) -> bool {
    if distance_to_blocks <= 0 {
        return true;
    }
    let pos = BlockPos::containing_vec(target);
    if !level.block_state(pos).is_air() {
        return false;
    }
    for dir in Direction::VALUES {
        for i in 1..distance_to_blocks {
            let offset = pos.relative_n(dir, i);
            if !level.block_state(offset).is_air() {
                return true;
            }
        }
    }
    false
}

fn choose_random_position(center: Vec3, random: &mut impl RandomSource) -> Vec3 {
    let x_target = center.x + ((random.next_float() * 2.0 - 1.0) * 16.0) as f64;
    let y_target = center.y + ((random.next_float() * 2.0 - 1.0) * 16.0) as f64;
    let z_target = center.z + ((random.next_float() * 2.0 - 1.0) * 16.0) as f64;
    Vec3::new(x_target, y_target, z_target)
}

fn choose_random_position_with_restriction(level: &mut ServerLevel, mob: EntityId, center: Vec3) -> Option<Vec3> {
    let target = choose_random_position(center, &mut level.entity_mut(mob).random);
    let m = level.mob(mob).expect("a mob");
    if m.has_home() && !m.is_within_home(BlockPos::containing_vec(target)) { None } else { Some(target) }
}

pub fn ghast_look_goal_tick(level: &mut ServerLevel, mob: EntityId) {
    face_movement_direction(level, mob);
}

#[derive(Default)]
pub struct GhastShootFireballGoal {
    pub charge_time: i32,
}

impl GhastShootFireballGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some()
    }

    pub fn start(&mut self) {
        self.charge_time = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_charging(level.entity_mut(mob), false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        if level.entity(target).distance_to_sqr(level.entity(mob)) < 4096.0 && level.has_line_of_sight(mob, target) {
            self.charge_time += 1;
            if self.charge_time == 10 && !level.entity(mob).is_silent() {
                let pos = level.entity(mob).block_position();
                level.level_event(None, SOUND_GHAST_WARNING, pos, 0);
            }
            if self.charge_time == 20 {
                let view_vector = level.get_view_vector(mob);
                let (t, g) = (level.entity(target), level.entity(mob));
                let xdd = t.x() - (g.x() + view_vector.x * 4.0);
                let ydd = t.get_y(0.5) - (0.5 + g.get_y(0.5));
                let zdd = t.z() - (g.z() + view_vector.z * 4.0);
                let direction = Vec3::new(xdd, ydd, zdd);
                if !level.entity(mob).is_silent() {
                    let pos = level.entity(mob).block_position();
                    level.level_event(None, SOUND_GHAST_FIREBALL, pos, 0);
                }
                let explosion_power = Ghast::get_explosion_power(level.mob(mob).expect("a ghast"));
                let mut entity = LargeFireball::from_mob(level, mob, direction.normalize(), explosion_power);
                let g = level.entity(mob);
                let position = Vec3::new(g.x() + view_vector.x * 4.0, g.get_y(0.5) + 0.5, entity.entity.z() + view_vector.z * 4.0);
                entity.entity.set_pos(position);
                level.add_fresh_entity(LevelEntity::Projectile(Box::new(entity)));
                self.charge_time = -40;
            }
        } else if self.charge_time > 0 {
            self.charge_time -= 1;
        }
        set_charging(level.entity_mut(mob), self.charge_time > 10);
    }
}
