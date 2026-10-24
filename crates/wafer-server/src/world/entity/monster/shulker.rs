use wafer_core::joml::Vector3f;
use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Direction, Vec3, mth};
use wafer_data::{block_tags, blocks, damage_type_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::block_util;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::look_control::{LookControl, rotate_towards};
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation, SHULKER_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::shulker_bullet::ShulkerBullet;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType, entity_types, sound_event};
use crate::world::item::dye_color::DyeColor;
use crate::world::level::BlockGetter;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::storage::tag_value_input;

const COVERED_ARMOR_MODIFIER_ID: &str = "minecraft:covered";
pub const DATA_ATTACH_FACE_ID: usize = 16;
pub const DATA_PEEK_ID: usize = 17;
pub const DATA_COLOR_ID: usize = 18;
const NO_COLOR: i8 = 16;
const DEFAULT_COLOR: i8 = 16;
const PEEK_PER_TICK: f32 = 0.05;
const DEFAULT_PEEK: i8 = 0;
const DEFAULT_ATTACH_FACE: Direction = Direction::Down;
const FORWARD: Vector3f = Vector3f::new(0.0, 0.0, 1.0);
const MAX_SCALE: f32 = 3.0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.0, 1.0).with_eye_height(0.5);

#[derive(Clone, Copy, Default, Debug)]
pub struct ShulkerPeek {
    pub current_peek_amount_o: f32,
    pub current_peek_amount: f32,
}

#[derive(Default)]
pub struct Shulker;

impl Shulker {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.extend([DataValue::Direction(DEFAULT_ATTACH_FACE), DataValue::Byte(DEFAULT_PEEK), DataValue::Byte(DEFAULT_COLOR)]);
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:shulker"), DIMENSIONS, data);
        entity.shulker_peek = Some(ShulkerPeek::default());
        let mut mob = Mob::new(entity, LivingEntity::new(&SHULKER_ATTRIBUTES), MobKind::Shulker(Shulker));
        mob.xp_reward = 5;
        mob.ai.look_control.kind = crate::world::entity::ai::control::look_control::LookControlKind::Shulker;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0).with_probability(0.02).only_horizontal()));
        goal_selector.add_goal(4, Goal::ShulkerAttack(ShulkerAttackGoal::default()));
        goal_selector.add_goal(7, Goal::ShulkerPeek(ShulkerPeekGoal::default()));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&["minecraft:shulker"]).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::ShulkerNearestAttack(ShulkerNearestAttackGoal::new()));
        target_selector.add_goal(3, Goal::ShulkerDefenseAttack(ShulkerDefenseAttackGoal::new()));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let attach_face = tag_value_input::get_i64(input, "AttachFace").map_or(DEFAULT_ATTACH_FACE, |id| Direction::from_3d_data_value(id as i32));
        set_attach_face(&mut mob.entity, attach_face);
        let byte = |key: &str, default: i8| tag_value_input::get_numeric(input, key).and_then(crate::world::level::block::entity::byte_value).unwrap_or(default);
        mob.entity.data.set(DATA_PEEK_ID, DataValue::Byte(byte("Peek", DEFAULT_PEEK)));
        mob.entity.data.set(DATA_COLOR_ID, DataValue::Byte(byte("Color", DEFAULT_COLOR)));
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("AttachFace".into(), Nbt::Byte(get_attach_face(&mob.entity) as i8)));
        output.push(("Peek".into(), Nbt::Byte(get_raw_peek_amount(&mob.entity) as i8)));
        output.push(("Color".into(), Nbt::Byte(get_color_byte(&mob.entity))));
    }
}

pub fn is_shulker(mob: &Mob) -> bool {
    matches!(mob.kind, MobKind::Shulker(_))
}

pub fn get_attach_face(entity: &Entity) -> Direction {
    match entity.data.get(DATA_ATTACH_FACE_ID) {
        DataValue::Direction(direction) => *direction,
        _ => DEFAULT_ATTACH_FACE,
    }
}

fn set_attach_face(entity: &mut Entity, attachment_direction: Direction) {
    entity.data.set(DATA_ATTACH_FACE_ID, DataValue::Direction(attachment_direction));
    let bb = entity.make_bounding_box(entity.position());
    entity.set_bounding_box(bb);
}

fn get_raw_peek_amount(entity: &Entity) -> i32 {
    match entity.data.get(DATA_PEEK_ID) {
        DataValue::Byte(peek) => *peek as i32,
        _ => 0,
    }
}

fn get_color_byte(entity: &Entity) -> i8 {
    match entity.data.get(DATA_COLOR_ID) {
        DataValue::Byte(color) => *color,
        _ => NO_COLOR,
    }
}

pub fn get_color(entity: &Entity) -> Option<DyeColor> {
    let color = get_color_byte(entity);
    (color != NO_COLOR && color <= 15).then(|| DyeColor::by_id(color as i32))
}

pub fn set_variant(entity: &mut Entity, color: Option<DyeColor>) {
    entity.data.set(DATA_COLOR_ID, DataValue::Byte(color.map_or(NO_COLOR, |color| color.id as i8)));
}

fn is_closed(entity: &Entity) -> bool {
    get_raw_peek_amount(entity) == 0
}

fn get_scale(entity: &Entity) -> f32 {
    entity.dimensions.width
}

pub fn sanitize_scale(scale: f32) -> f32 {
    scale.min(MAX_SCALE)
}

fn get_physical_peek(amount: f32) -> f32 {
    0.5 - mth::sin(((0.5 + amount) * std::f32::consts::PI) as f64) * 0.5
}

pub fn make_bounding_box(entity: &Entity, position: Vec3) -> AABB {
    let phys_peek = get_physical_peek(entity.shulker_peek.expect("a shulker").current_peek_amount);
    let direction = get_attach_face(entity).opposite();
    get_progress_aabb(get_scale(entity), direction, phys_peek, position)
}

pub fn get_default_dimensions(entity: &Entity) -> EntityDimensions {
    let current_peek_amount = entity.shulker_peek.expect("a shulker").current_peek_amount;
    if get_attach_face(entity) == Direction::Down && current_peek_amount > 0.0 {
        let height_scale_factor = 1.0 + current_peek_amount;
        DIMENSIONS.scale_xy(1.0, height_scale_factor)
    } else {
        DIMENSIONS
    }
}

pub fn get_progress_aabb(size: f32, direction: Direction, progress_to: f32, position: Vec3) -> AABB {
    get_progress_delta_aabb(size, direction, -1.0, progress_to, position)
}

pub fn get_progress_delta_aabb(size: f32, direction: Direction, progress_from: f32, progress_to: f32, position: Vec3) -> AABB {
    let size_d = size as f64;
    let bounds_at_bottom_center = AABB::new(-size_d * 0.5, 0.0, -size_d * 0.5, size_d * 0.5, size_d, size_d * 0.5);
    let max_movement = progress_from.max(progress_to) as f64;
    let min_movement = progress_from.min(progress_to) as f64;
    let (step_x, step_y, step_z) = (direction.step_x() as f64, direction.step_y() as f64, direction.step_z() as f64);
    let aabb = bounds_at_bottom_center.expand_towards(step_x * max_movement * size_d, step_y * max_movement * size_d, step_z * max_movement * size_d).contract(
        -step_x * (1.0 + min_movement) * size_d,
        -step_y * (1.0 + min_movement) * size_d,
        -step_z * (1.0 + min_movement) * size_d,
    );
    aabb.move_by(position.x, position.y, position.z)
}

pub fn set_pos(entity: &mut Entity, x: f64, y: f64, z: f64) {
    let old_pos = entity.block_position();
    if entity.vehicle.is_some() {
        entity.entity_set_pos(Vec3::new(x, y, z));
    } else {
        entity.entity_set_pos(Vec3::new(mth::floor(x) as f64 + 0.5, mth::floor(y + 0.5) as f64, mth::floor(z) as f64 + 0.5));
    }
    if entity.tick_count != 0 && entity.block_position() != old_pos {
        entity.data.set(DATA_PEEK_ID, DataValue::Byte(0));
        entity.needs_sync = true;
    }
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.shulker.ambient"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.shulker.death"
}

pub fn get_hurt_sound(entity: &Entity) -> &'static str {
    if is_closed(entity) { "minecraft:entity.shulker.hurt_closed" } else { "minecraft:entity.shulker.hurt" }
}

pub fn get_max_head_x_rot() -> i32 {
    180
}

pub fn get_max_head_y_rot() -> i32 {
    180
}

impl ServerLevel {
    pub fn is_shulker(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(is_shulker)
    }

    pub fn shulker_play_ambient_sound(&mut self, id: EntityId) {
        if !is_closed(self.entity(id)) {
            self.mob_play_ambient_sound(id);
        }
    }

    pub fn shulker_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.mob(id).is_none() {
            return;
        }
        let e = self.entity(id);
        let (block_position, attach_face) = (e.block_position(), get_attach_face(e));
        if !self.is_passenger(id) && !self.shulker_can_stay_at(id, block_position, attach_face) {
            self.shulker_find_new_attachment(id);
        }
        if self.shulker_update_peek_amount(id) {
            self.shulker_on_peek_amount_change(id);
        }
    }

    fn shulker_find_new_attachment(&mut self, id: EntityId) {
        let block_position = self.entity(id).block_position();
        match self.shulker_find_attachable_surface(id, block_position) {
            Some(attachment_direction) => set_attach_face(self.entity_mut(id), attachment_direction),
            None => {
                self.shulker_teleport_somewhere(id);
            }
        }
    }

    fn shulker_update_peek_amount(&mut self, id: EntityId) -> bool {
        let e = self.entity_mut(id);
        let target_peek_amount = get_raw_peek_amount(e) as f32 * 0.01;
        let peek = e.shulker_peek.as_mut().expect("a shulker");
        peek.current_peek_amount_o = peek.current_peek_amount;
        if peek.current_peek_amount == target_peek_amount {
            return false;
        }
        if peek.current_peek_amount > target_peek_amount {
            peek.current_peek_amount = mth::clamp_f(peek.current_peek_amount - PEEK_PER_TICK, target_peek_amount, 1.0);
        } else {
            peek.current_peek_amount = mth::clamp_f(peek.current_peek_amount + PEEK_PER_TICK, 0.0, target_peek_amount);
        }
        true
    }

    fn shulker_on_peek_amount_change(&mut self, id: EntityId) {
        self.entity_mut(id).reapply_position();
        self.refresh_mob_dimensions(id);
        let e = self.entity(id);
        let peek = e.shulker_peek.expect("a shulker");
        let physical_peek = get_physical_peek(peek.current_peek_amount);
        let physical_peek_old = get_physical_peek(peek.current_peek_amount_o);
        let direction = get_attach_face(e).opposite();
        let scale = get_scale(e);
        let push = (physical_peek - physical_peek_old) * scale;
        if push <= 0.0 {
            return;
        }
        let area = get_progress_delta_aabb(scale, direction, physical_peek_old, physical_peek, e.position());
        let entities = self.get_entities_matching(Some(id), &area, |level, entity| !level.is_spectator(entity) && !level.is_passenger_of_same_vehicle(entity, id));
        for entity in entities {
            if !self.is_shulker(entity) && !self.entity(entity).no_physics {
                let delta = Vec3::new((push * direction.step_x() as f32) as f64, (push * direction.step_y() as f32) as f64, (push * direction.step_z() as f32) as f64);
                self.move_entity(entity, MoverType::Shulker, delta);
            }
        }
    }

    pub fn shulker_start_riding(&mut self, id: EntityId) {
        set_attach_face(self.entity_mut(id), Direction::Down);
    }

    pub fn shulker_stop_riding(&mut self, id: EntityId) {
        let living = self.living_mut(id);
        living.y_body_rot_o = 0.0;
        living.y_body_rot = 0.0;
    }

    pub fn shulker_finalize_spawn(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.set_y_rot(0.0);
        e.y_head_rot = e.y_rot;
        e.set_old_pos_and_rot();
    }

    pub fn shulker_move(&mut self, id: EntityId, mover_type: MoverType, delta: Vec3) {
        if mover_type == MoverType::ShulkerBox {
            self.shulker_teleport_somewhere(id);
        } else {
            self.entity_move(id, mover_type, delta);
        }
    }

    fn shulker_find_attachable_surface(&mut self, id: EntityId, target: BlockPos) -> Option<Direction> {
        Direction::VALUES.into_iter().find(|&direction| self.shulker_can_stay_at(id, target, direction))
    }

    fn shulker_can_stay_at(&mut self, id: EntityId, target: BlockPos, face: Direction) -> bool {
        if self.shulker_is_position_blocked(id, target) {
            return false;
        }
        let opposite_face = face.opposite();
        if !self.loaded_and_entity_can_stand_on_face(target.relative(face), id, opposite_face) {
            return false;
        }
        let fully_opened = get_progress_aabb(get_scale(self.entity(id)), opposite_face, 1.0, Vec3::at_bottom_center_of(target)).deflate_all(1.0e-6);
        self.no_collision_for(id, &fully_opened)
    }

    fn shulker_is_position_blocked(&mut self, id: EntityId, target: BlockPos) -> bool {
        let state = self.block_state(target);
        if state.is_air() {
            return false;
        }
        let moving_piston_in_our_current_position = state.is_block(blocks::MOVING_PISTON) && target == self.entity(id).block_position();
        !moving_piston_in_our_current_position
    }

    pub fn loaded_and_entity_can_stand_on_face(&mut self, pos: BlockPos, entity: EntityId, face: Direction) -> bool {
        if self.chunks.is_outside_build_height(pos.y) || !self.has_chunk_at(pos) {
            return false;
        }
        let state = self.block_state(pos);
        let context = CollisionContext::of(self, entity);
        let shape = self.get_state_collision_shape_in_context(state, pos, &context);
        wafer_core::shapes::is_face_full(&shape, face)
    }

    pub fn shulker_teleport_somewhere(&mut self, id: EntityId) -> bool {
        if self.mob(id).expect("a shulker").is_no_ai() || !self.is_alive(id) {
            return false;
        }
        let old_pos = self.entity(id).block_position();
        for _ in 0..5 {
            let random = &mut self.entity_mut(id).random;
            let dx = mth::random_between_inclusive(random, -8, 8);
            let dy = mth::random_between_inclusive(random, -8, 8);
            let dz = mth::random_between_inclusive(random, -8, 8);
            let target = old_pos.offset(dx, dy, dz);
            if target.y > self.chunks.min_y()
                && self.block_state(target).is_air()
                && self.world_border.is_within_bounds_pos(target)
                && self.no_collision_for(id, &AABB::of_block(target).deflate_all(1.0e-6))
                && let Some(attachment_direction) = self.shulker_find_attachable_surface(id, target)
                && !self.block_state(target.relative(attachment_direction)).is(&block_tags::SHULKER_DOES_NOT_TELEPORT_TO)
            {
                self.un_ride(id);
                set_attach_face(self.entity_mut(id), attachment_direction);
                self.entity_play_sound(id, sound_event("minecraft:entity.shulker.teleport"), 1.0, 1.0);
                self.entity_mut(id).set_pos(Vec3::new(target.x as f64 + 0.5, target.y as f64, target.z as f64 + 0.5));
                self.game_event_at_with(crate::world::level::gameevent::game_event::TELEPORT, old_pos, crate::world::level::gameevent::game_event::Context::of(Some(id)));
                let packed_diff = block_util::pack_difference_in_position(old_pos, self.entity(id).block_position(), 8, 8, 8);
                self.level_event(None, crate::server::level::events::PARTICLES_SHULKER_TELEPORT, old_pos, packed_diff);
                self.entity_mut(id).data.set(DATA_PEEK_ID, DataValue::Byte(0));
                self.set_target(id, None);
                return true;
            }
        }
        false
    }

    pub fn shulker_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if is_closed(self.entity(id)) && source.direct_entity.is_some_and(|direct| self.as_projectile(direct).is_some_and(|projectile| projectile.abstract_arrow().is_some())) {
            return false;
        }
        if !self.living_entity_hurt_server(id, source.clone(), damage) {
            return false;
        }
        let mob = self.mob_mut(id).expect("a shulker");
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        if (mob.health as f64) < max_health as f64 * 0.5 && mob.entity.random.next_int_bounded(4) == 0 {
            self.shulker_teleport_somewhere(id);
        } else if source.is(&damage_type_tags::IS_PROJECTILE)
            && let Some(direct_entity) = source.direct_entity
            && self.entity_ref(direct_entity).is_some_and(|e| e.entity_type == entity_types::SHULKER_BULLET.id)
        {
            self.shulker_hit_by_shulker_bullet(id);
        }
        true
    }

    fn shulker_hit_by_shulker_bullet(&mut self, id: EntityId) {
        let e = self.entity(id);
        let (old_position, old_aabb) = (e.position(), e.bounding_box());
        if is_closed(e) || !self.shulker_teleport_somewhere(id) {
            return;
        }
        let shulker_count = self.get_entities_matching(None, &old_aabb.inflate_all(8.0), |level, entity| level.is_shulker(entity) && level.is_alive(entity)).len();
        let failure_chance = (shulker_count as i32 - 1) as f32 / 5.0;
        if self.random.next_float() < failure_chance {
            return;
        }
        let mut baby = Shulker::create();
        set_variant(&mut baby.entity, get_color(self.entity(id)));
        let (y_rot, x_rot) = (baby.entity.y_rot, baby.entity.x_rot);
        baby.entity.snap_to(old_position.x, old_position.y, old_position.z, y_rot, x_rot);
        self.add_fresh_entity(LevelEntity::Mob(Box::new(baby)));
    }

    fn shulker_set_raw_peek_amount(&mut self, id: EntityId, amount: i32) {
        let mob = self.mob_mut(id).expect("a shulker");
        let armor = mob.living.attributes.instance(Attribute::Armor).expect("shulkers have armor");
        armor.remove_modifier(COVERED_ARMOR_MODIFIER_ID);
        if amount == 0 {
            armor.add_permanent_modifier(AttributeModifier { id: std::borrow::Cow::Borrowed(COVERED_ARMOR_MODIFIER_ID), amount: 20.0, operation: Operation::AddValue });
            self.entity_play_sound(id, sound_event("minecraft:entity.shulker.close"), 1.0, 1.0);
            self.entity_game_event(id, crate::world::level::gameevent::game_event::CONTAINER_CLOSE);
        } else {
            self.entity_play_sound(id, sound_event("minecraft:entity.shulker.open"), 1.0, 1.0);
            self.entity_game_event(id, crate::world::level::gameevent::game_event::CONTAINER_OPEN);
        }
        self.entity_mut(id).data.set(DATA_PEEK_ID, DataValue::Byte(amount as i8));
    }

    pub fn shulker_look_control_tick(&mut self, id: EntityId) {
        self.entity_mut(id).set_x_rot(0.0);
        let mob = self.mob_mut(id).expect("a shulker");
        let look = &mut mob.ai.look_control;
        let e = &mut mob.entity;
        if look.look_at_cooldown > 0 {
            look.look_at_cooldown -= 1;
            if let Some(y_rot_d) = shulker_look_control_get_y_rot_d(look, e) {
                e.y_head_rot = rotate_towards(e.y_head_rot, y_rot_d, look.y_max_rot_speed);
            }
            let x_rot = rotate_towards(e.x_rot, 0.0, look.x_max_rot_angle);
            e.set_x_rot(x_rot);
        } else {
            e.y_head_rot = rotate_towards(e.y_head_rot, mob.living.y_body_rot, 10.0);
        }
    }
}

fn shulker_look_control_get_y_rot_d(look: &LookControl, e: &Entity) -> Option<f32> {
    let attach_face = get_attach_face(e).opposite();
    let forward = attach_face.get_rotation().transform(FORWARD);
    let up_normal = Vector3f::new(attach_face.step_x() as f32, attach_face.step_y() as f32, attach_face.step_z() as f32);
    let right = up_normal.cross(forward);
    let xd = look.wanted_x - e.x();
    let yd = look.wanted_y - e.eye_y();
    let zd = look.wanted_z - e.z();
    let out = Vector3f::new(xd as f32, yd as f32, zd as f32);
    let delta_right = right.dot(out);
    let delta_forward = forward.dot(out);
    if !(delta_right.abs() > 1.0e-5) && !(delta_forward.abs() > 1.0e-5) {
        None
    } else {
        Some((mth::atan2(-delta_right as f64, delta_forward as f64) * mth::RAD_TO_DEG as f64) as f32)
    }
}

#[derive(Default)]
pub struct ShulkerAttackGoal {
    attack_time: i32,
}

impl ShulkerAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match level.get_target(mob) {
            Some(target) if level.is_alive(target) => level.difficulty != Difficulty::Peaceful,
            _ => false,
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.attack_time = 20;
        level.shulker_set_raw_peek_amount(mob, 100);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.shulker_set_raw_peek_amount(mob, 0);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.difficulty == Difficulty::Peaceful {
            return;
        }
        self.attack_time -= 1;
        let Some(target) = level.get_target(mob) else { return };
        level.set_look_at_entity(mob, target, 180.0, 180.0);
        let distance = level.entity(mob).distance_to_sqr(level.entity(target));
        if distance < 400.0 {
            if self.attack_time <= 0 {
                let random = &mut level.entity_mut(mob).random;
                self.attack_time = 20 + random.next_int_bounded(10) * 20 / 2;
                let axis = get_attach_face(level.entity(mob)).axis();
                let bullet = ShulkerBullet::from_owner(level, mob, target, axis);
                level.add_fresh_entity(LevelEntity::Projectile(Box::new(bullet)));
                let random = &mut level.entity_mut(mob).random;
                let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
                level.entity_play_sound(mob, sound_event("minecraft:entity.shulker.shoot"), 2.0, pitch);
            }
        } else {
            level.set_target(mob, None);
        }
    }
}

fn get_target_search_area(level: &ServerLevel, mob: EntityId, follow_distance: f64) -> AABB {
    let e = level.entity(mob);
    match get_attach_face(e).axis() {
        wafer_core::Axis::X => e.bounding_box().inflate(4.0, follow_distance, follow_distance),
        wafer_core::Axis::Z => e.bounding_box().inflate(follow_distance, follow_distance, 4.0),
        wafer_core::Axis::Y => e.bounding_box().inflate(follow_distance, 4.0, follow_distance),
    }
}

fn is_enemy(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_enemy(target)
}

pub struct ShulkerDefenseAttackGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
}

impl ShulkerDefenseAttackGoal {
    fn new() -> ShulkerDefenseAttackGoal {
        let mut nearest_attackable_target_goal = NearestAttackableTargetGoal::with(TargetType::LivingEntity, 10, true, false, Some(is_enemy));
        nearest_attackable_target_goal.target_search_area = Some(get_target_search_area);
        ShulkerDefenseAttackGoal { nearest_attackable_target_goal }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.team_of(mob).is_none() { false } else { self.nearest_attackable_target_goal.can_use(level, mob) }
    }
}

pub struct ShulkerNearestAttackGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
}

impl ShulkerNearestAttackGoal {
    fn new() -> ShulkerNearestAttackGoal {
        let mut nearest_attackable_target_goal = NearestAttackableTargetGoal::new(TargetType::Player, true);
        nearest_attackable_target_goal.target_search_area = Some(get_target_search_area);
        ShulkerNearestAttackGoal { nearest_attackable_target_goal }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.difficulty == Difficulty::Peaceful { false } else { self.nearest_attackable_target_goal.can_use(level, mob) }
    }
}

#[derive(Default)]
pub struct ShulkerPeekGoal {
    peek_time: i32,
}

impl ShulkerPeekGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_some() || level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(40)) != 0 {
            return false;
        }
        let e = level.entity(mob);
        let (block_position, attach_face) = (e.block_position(), get_attach_face(e));
        level.shulker_can_stay_at(mob, block_position, attach_face)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_none() && self.peek_time > 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let random = &mut level.entity_mut(mob).random;
        self.peek_time = reduced_tick_delay(20 * (1 + random.next_int_bounded(3)));
        level.shulker_set_raw_peek_amount(mob, 30);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.get_target(mob).is_none() {
            level.shulker_set_raw_peek_amount(mob, 0);
        }
    }

    pub fn tick(&mut self) {
        self.peek_time -= 1;
    }
}
