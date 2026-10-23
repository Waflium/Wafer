use std::collections::VecDeque;
use std::sync::atomic::{AtomicI32, Ordering};

use wafer_core::hash_map::FastSet;
use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use wafer_core::{AABB, Axis, AxisDirection, BlockPos, ChunkPos, Direction, Vec3, mth};
use wafer_data::{BlockState, block_tags, blocks, damage_type_tags, damage_types, entity_type_tags, fluid_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::attributes::Attribute;
use super::entity_attachments::EntityAttachment;
use super::{EntityDimensions, ItemEntity, LevelEntity, data, relative};
use crate::world::damagesource::DamageSource;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::{InteractionHand, InteractionResult};
use super::data::{DataValue, Pose, SynchedEntityData, ids};
use super::fluid_interaction::{CurrentFluid, EntityFluidInteraction};
use super::inside_block::{InsideBlockEffectType, InsideBlockEffects, QueuedEffect};
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes;
use crate::world::level::BlockGetter;
use crate::world::level::gameevent::game_event::{self, Context};
use crate::world::level::block;
use crate::world::level::block::state::block_behaviour::{self, RenderShape};
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::world::level::block::portal::{PendingPortalTeleport, Portal, PortalEntity};
use crate::world::phys::shapes::collision_context::CollisionContext;
use super::portal_processor::PortalProcessor;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::clip::for_each_block_intersected_between;
use crate::world::level::collision::{Collider, collide_with_shapes};

pub type EntityId = i32;

pub fn next_entity_id() -> EntityId {
    static COUNTER: AtomicI32 = AtomicI32::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemovalReason {
    Killed,
    Discarded,
    UnloadedToChunk,
    UnloadedWithPlayer,
    ChangedDimension,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoverType {
    SelfMove,
    Player,
    Piston,
    ShulkerBox,
    Shulker,
}

impl MoverType {
    pub fn is_server_and_client_simulated(self) -> bool {
        self != MoverType::SelfMove
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MovementEmission {
    None,
    Events,
    All,
}

impl MovementEmission {
    pub fn emits_anything(self) -> bool {
        self != MovementEmission::None
    }

    pub fn emits_sounds(self) -> bool {
        self == MovementEmission::All
    }

    pub fn emits_events(self) -> bool {
        self == MovementEmission::Events || self == MovementEmission::All
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Movement {
    pub from: Vec3,
    pub to: Vec3,
    pub axis_dependent_original_movement: Option<Vec3>,
}

pub mod flags {
    pub const ON_FIRE: u8 = 0;
    pub const SHIFT_KEY_DOWN: u8 = 1;
    pub const SPRINTING: u8 = 3;
    pub const SWIMMING: u8 = 4;
    pub const INVISIBLE: u8 = 5;
    pub const GLOWING: u8 = 6;
    pub const FALL_FLYING: u8 = 7;
}

pub const MAX_ENTITY_TAG_COUNT: usize = 1024;
pub const TOTAL_AIR_SUPPLY: i32 = 300;

pub struct Entity {
    pub id: EntityId,
    pub uuid: u128,
    pub entity_type: i32,
    pub random: LegacyRandomSource,
    pos: Vec3,
    bb: AABB,
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub y_rot_o: f32,
    pub x_rot_o: f32,
    pub y_head_rot: f32,
    pub dimensions: EntityDimensions,
    pub data: SynchedEntityData,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub requires_precise_position: bool,
    pub vertical_collision: bool,
    pub vertical_collision_below: bool,
    pub minor_horizontal_collision: bool,
    pub tick_count: i32,
    pub removal: Option<RemovalReason>,
    pub needs_sync: bool,
    pub sync_position: bool,
    pub sync_velocity: bool,
    pub xo: Vec3,
    pub old_pos: Vec3,
    pub stuck_speed_multiplier: Vec3,
    pub move_dist: f32,
    pub fly_dist: f32,
    pub fall_distance: f64,
    pub next_step: f32,
    pub no_physics: bool,
    pub first_tick: bool,
    pub pending_on_move: bool,
    pub remaining_fire_ticks: i32,
    pub invulnerable: bool,
    pub invulnerable_time: i32,
    pub portal_cooldown: i32,
    pub portal_process: Option<super::portal_processor::PortalProcessor>,
    pub has_glowing_tag: bool,
    pub has_visual_fire: bool,
    pub tags: Vec<String>,
    pub custom_data: Option<Nbt>,
    pub was_touching_water: bool,
    pub was_eye_in_water: bool,
    pub is_in_powder_snow: bool,
    pub was_in_powder_snow: bool,
    pub fluid_interaction: EntityFluidInteraction,
    pub main_supporting_block_pos: Option<BlockPos>,
    pub on_ground_no_blocks: bool,
    pub crystal_sound_intensity: f32,
    pub last_crystal_sound_play_tick: i32,
    pub last_known_speed: Vec3,
    pub last_known_position: Option<Vec3>,
    pub in_block_state: Option<BlockState>,
    pub movement_this_tick: VecDeque<Movement>,
    piston_deltas: [f64; 3],
    piston_deltas_game_time: i64,
    pub final_movements_this_tick: Vec<Movement>,
    pub inside_effects: InsideBlockEffects,
    pub passengers: Vec<EntityId>,
    pub vehicle: Option<EntityId>,
    pub boarding_cooldown: i32,
    pub block_attached: Option<super::decoration::block_attached_entity::BlockAttachedEntity>,
    pub shulker_peek: Option<super::monster::shulker::ShulkerPeek>,
}

impl Entity {
    pub fn new(entity_type: i32, dimensions: EntityDimensions, data: Vec<DataValue>) -> Entity {
        let mut random = LegacyRandomSource::create();
        let uuid = create_insecure_uuid(&mut random);
        Entity {
            id: next_entity_id(),
            uuid,
            entity_type,
            random,
            pos: Vec3::ZERO,
            bb: dimensions.make_bounding_box(Vec3::ZERO),
            delta_movement: Vec3::ZERO,
            y_rot: 0.0,
            x_rot: 0.0,
            y_rot_o: 0.0,
            x_rot_o: 0.0,
            y_head_rot: 0.0,
            dimensions,
            data: SynchedEntityData::new(data),
            on_ground: false,
            horizontal_collision: false,
            vertical_collision: false,
            vertical_collision_below: false,
            minor_horizontal_collision: false,
            tick_count: 0,
            removal: None,
            needs_sync: false,
            requires_precise_position: false,
            sync_position: false,
            sync_velocity: false,
            xo: Vec3::ZERO,
            old_pos: Vec3::ZERO,
            stuck_speed_multiplier: Vec3::ZERO,
            move_dist: 0.0,
            fly_dist: 0.0,
            fall_distance: 0.0,
            next_step: 1.0,
            no_physics: false,
            first_tick: true,
            pending_on_move: false,
            remaining_fire_ticks: 0,
            invulnerable: false,
            invulnerable_time: 0,
            portal_cooldown: 0,
            portal_process: None,
            has_glowing_tag: false,
            has_visual_fire: false,
            tags: Vec::new(),
            custom_data: None,
            was_touching_water: false,
            was_eye_in_water: false,
            is_in_powder_snow: false,
            was_in_powder_snow: false,
            fluid_interaction: EntityFluidInteraction::default(),
            main_supporting_block_pos: None,
            on_ground_no_blocks: false,
            crystal_sound_intensity: 0.0,
            last_crystal_sound_play_tick: 0,
            last_known_speed: Vec3::ZERO,
            last_known_position: None,
            in_block_state: None,
            movement_this_tick: VecDeque::new(),
            piston_deltas: [0.0; 3],
            piston_deltas_game_time: 0,
            final_movements_this_tick: Vec::new(),
            inside_effects: InsideBlockEffects::default(),
            passengers: Vec::new(),
            vehicle: None,
            boarding_cooldown: 0,
            block_attached: None,
            shulker_peek: None,
        }
    }

    pub fn position(&self) -> Vec3 {
        self.pos
    }

    pub fn x(&self) -> f64 {
        self.pos.x
    }

    pub fn y(&self) -> f64 {
        self.pos.y
    }

    pub fn z(&self) -> f64 {
        self.pos.z
    }

    pub fn block_attached_pos(&self) -> BlockPos {
        self.block_attached.map_or(BlockPos::ZERO, |block_attached| block_attached.pos)
    }

    pub fn set_pos(&mut self, pos: Vec3) {
        if self.block_attached.is_some() {
            super::decoration::block_attached_entity::set_pos(self, pos.x, pos.y, pos.z);
            return;
        }
        if self.shulker_peek.is_some() {
            super::monster::shulker::set_pos(self, pos.x, pos.y, pos.z);
            return;
        }
        self.entity_set_pos(pos);
    }

    pub fn entity_set_pos(&mut self, pos: Vec3) {
        self.set_pos_raw(pos);
        self.bb = self.make_bounding_box(pos);
    }

    pub fn make_bounding_box(&self, position: Vec3) -> AABB {
        if self.shulker_peek.is_some() {
            super::monster::shulker::make_bounding_box(self, position)
        } else if self.entity_type == super::entity_types::WIND_CHARGE.id || self.entity_type == super::entity_types::BREEZE_WIND_CHARGE.id {
            super::projectile::hurtingprojectile::windcharge::abstract_wind_charge::make_bounding_box(self.dimensions, position)
        } else {
            self.dimensions.make_bounding_box(position)
        }
    }

    pub fn set_pos_raw(&mut self, pos: Vec3) {
        if self.pos != pos {
            if BlockPos::containing_vec(pos) != BlockPos::containing_vec(self.pos) {
                self.in_block_state = None;
            }
            self.pos = pos;
            self.pending_on_move = !self.first_tick;
        }
    }

    pub fn reapply_position(&mut self) {
        self.last_known_position = None;
        self.set_pos(self.pos);
    }

    pub fn bounding_box(&self) -> AABB {
        self.bb
    }

    pub fn set_bounding_box(&mut self, bb: AABB) {
        self.bb = bb;
    }

    pub fn block_position(&self) -> BlockPos {
        BlockPos::containing_vec(self.pos)
    }

    pub fn chunk_position(&self) -> ChunkPos {
        ChunkPos::new(mth::floor(self.pos.x) >> 4, mth::floor(self.pos.z) >> 4)
    }

    pub fn eye_y(&self) -> f64 {
        self.pos.y + self.dimensions.eye_height as f64
    }

    pub fn eye_position(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.eye_y(), self.pos.z)
    }

    pub fn add_tag(&mut self, tag: &str) -> bool {
        if self.tags.len() >= MAX_ENTITY_TAG_COUNT || self.tags.iter().any(|t| t == tag) {
            return false;
        }
        self.tags.push(tag.to_string());
        true
    }

    pub fn remove_tag(&mut self, tag: &str) -> bool {
        let Some(i) = self.tags.iter().position(|t| t == tag) else { return false };
        self.tags.remove(i);
        true
    }

    pub fn is_removed(&self) -> bool {
        self.removal.is_some()
    }

    pub fn is_invulnerable(&self) -> bool {
        self.is_permanently_invulnerable() || self.is_temporarily_invulnerable()
    }

    pub fn is_permanently_invulnerable(&self) -> bool {
        self.invulnerable
    }

    pub fn is_temporarily_invulnerable(&self) -> bool {
        self.invulnerable_time > 0
    }

    pub fn random_x(&mut self, spread: f64) -> f64 {
        self.pos.x + self.dimensions.width as f64 * ((2.0 * self.random.next_double() - 1.0) * spread)
    }

    pub fn random_y(&mut self) -> f64 {
        self.pos.y + self.dimensions.height as f64 * self.random.next_double()
    }

    pub fn random_y_scaled(&mut self, spread: f64) -> f64 {
        self.pos.y + self.dimensions.height as f64 * ((2.0 * self.random.next_double() - 1.0) * spread)
    }

    pub fn get_y(&self, scale: f64) -> f64 {
        self.pos.y + self.dimensions.height as f64 * scale
    }

    pub fn random_z(&mut self, spread: f64) -> f64 {
        self.pos.z + self.dimensions.width as f64 * ((2.0 * self.random.next_double() - 1.0) * spread)
    }

    pub fn custom_name(&self) -> Option<crate::network::chat::Component> {
        match self.data.get(data::ids::CUSTOM_NAME) {
            DataValue::OptionalComponent(Some(name)) => Some(crate::network::chat::Component::from_nbt(name)),
            _ => None,
        }
    }

    pub fn has_custom_name(&self) -> bool {
        matches!(self.data.get(data::ids::CUSTOM_NAME), DataValue::OptionalComponent(Some(_)))
    }

    pub fn set_custom_name(&mut self, name: Option<Nbt>) {
        let name = name.and_then(|name| crate::network::chat::component_serialization::decode(&name).ok()).map(|name| name.to_nbt());
        self.data.set(data::ids::CUSTOM_NAME, DataValue::OptionalComponent(name));
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        for ty in [dc::CUSTOM_NAME, dc::CUSTOM_DATA] {
            if let Some(value) = stack.get(ty) {
                self.apply_implicit_component(ty, value);
            }
        }
    }

    pub fn apply_implicit_component(&mut self, ty: dc::DataComponentType, value: &Nbt) -> bool {
        if ty == dc::CUSTOM_NAME {
            self.set_custom_name(Some(value.clone()));
            true
        } else if ty == dc::CUSTOM_DATA {
            self.custom_data = Some(value.clone());
            true
        } else {
            false
        }
    }

    pub fn discard(&mut self) {
        self.removal = Some(RemovalReason::Discarded);
    }

    pub fn view_vector(&self) -> Vec3 {
        Vec3::direction_from_rotation(self.x_rot, self.y_rot)
    }

    pub fn look_angle(&self) -> Vec3 {
        let x_rot = self.x_rot * mth::DEG_TO_RAD;
        let y_rot = -self.y_rot * mth::DEG_TO_RAD;
        let (y_cos, y_sin) = (mth::cos(y_rot as f64), mth::sin(y_rot as f64));
        let (x_cos, x_sin) = (mth::cos(x_rot as f64), mth::sin(x_rot as f64));
        Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
    }

    pub fn set_delta_movement(&mut self, movement: Vec3) {
        if self.shulker_peek.is_some() {
            return;
        }
        if movement.is_finite() {
            self.delta_movement = movement;
        }
    }

    pub fn add_delta_movement(&mut self, momentum: Vec3) {
        if momentum.is_finite() {
            self.set_delta_movement(self.delta_movement.add_vec(momentum));
        }
    }

    pub fn set_old_pos(&mut self) {
        self.xo = self.pos;
        self.old_pos = self.pos;
        self.y_rot_o = self.y_rot;
        self.x_rot_o = self.x_rot;
    }

    pub fn set_old_rot(&mut self) {
        self.y_rot_o = self.y_rot;
        self.x_rot_o = self.x_rot;
    }

    pub fn set_old_pos_and_rot(&mut self) {
        self.set_old_pos();
        self.set_old_rot();
    }

    pub fn old_position(&self) -> Vec3 {
        self.old_pos
    }

    pub fn push(&mut self, xa: f64, ya: f64, za: f64) {
        if xa.is_finite() && ya.is_finite() && za.is_finite() {
            self.set_delta_movement(self.delta_movement.add(xa, ya, za));
            self.needs_sync = true;
        }
    }

    pub fn snap_to(&mut self, x: f64, y: f64, z: f64, y_rot: f32, x_rot: f32) {
        self.set_pos_raw(Vec3::new(x, y, z));
        self.set_y_rot(y_rot);
        self.set_x_rot(x_rot);
        self.set_old_pos_and_rot();
        self.reapply_position();
    }

    pub fn abs_snap_to(&mut self, x: f64, y: f64, z: f64) {
        let cx = mth::clamp(x, -3.0e7, 3.0e7);
        let cz = mth::clamp(z, -3.0e7, 3.0e7);
        self.xo = Vec3::new(cx, y, cz);
        self.set_pos(Vec3::new(cx, y, cz));
    }

    pub fn teleport_set_position(&mut self, current: relative::PositionMoveRotation, destination: relative::PositionMoveRotation, relatives: u16) {
        let absolute = relative::PositionMoveRotation::calculate_absolute(current, destination, relatives);
        self.set_pos_raw(absolute.position);
        self.set_y_rot(absolute.y_rot);
        self.set_x_rot(absolute.x_rot);
        self.reapply_position();
        self.set_old_pos_and_rot();
        self.set_delta_movement(absolute.delta_movement);
        self.clear_movement_this_tick();
    }

    pub fn abs_snap_rotation_to(&mut self, y_rot: f32, x_rot: f32) {
        self.set_y_rot(y_rot % 360.0);
        self.set_x_rot(mth::clamp_f(x_rot, -90.0, 90.0) % 360.0);
        self.set_old_rot();
    }

    pub fn set_y_rot(&mut self, y_rot: f32) {
        if y_rot.is_finite() {
            self.y_rot = y_rot;
        }
    }

    pub fn set_x_rot(&mut self, x_rot: f32) {
        if x_rot.is_finite() {
            self.x_rot = (x_rot % 360.0).clamp(-90.0, 90.0);
        }
    }

    pub fn set_rot(&mut self, y_rot: f32, x_rot: f32) {
        self.set_y_rot(y_rot % 360.0);
        self.set_x_rot(x_rot % 360.0);
    }

    fn shared_flags(&self) -> i8 {
        match self.data.get(ids::SHARED_FLAGS) {
            DataValue::Byte(b) => *b,
            _ => 0,
        }
    }

    pub fn shared_flag(&self, flag: u8) -> bool {
        self.shared_flags() & (1 << flag) != 0
    }

    pub fn set_shared_flag(&mut self, flag: u8, value: bool) {
        let current = self.shared_flags();
        let new = if value { current | 1 << flag } else { current & !(1 << flag) };
        self.data.set(ids::SHARED_FLAGS, DataValue::Byte(new));
    }

    pub fn is_shift_key_down(&self) -> bool {
        self.shared_flag(flags::SHIFT_KEY_DOWN)
    }

    pub fn set_shift_key_down(&mut self, value: bool) {
        self.set_shared_flag(flags::SHIFT_KEY_DOWN, value);
    }

    pub fn is_suppressing_bounce(&self) -> bool {
        self.is_shift_key_down()
    }

    pub fn is_discrete(&self) -> bool {
        self.is_shift_key_down()
    }

    pub fn get_direction(&self) -> wafer_core::Direction {
        wafer_core::Direction::from_y_rot(self.y_rot as f64)
    }

    pub fn get_motion_direction(&self) -> wafer_core::Direction {
        self.get_direction()
    }

    pub fn distance_to_sqr_xyz(&self, x: f64, y: f64, z: f64) -> f64 {
        let xd = self.x() - x;
        let yd = self.y() - y;
        let zd = self.z() - z;
        xd * xd + yd * yd + zd * zd
    }

    pub fn distance_to_sqr(&self, entity: &Entity) -> f64 {
        self.position().distance_to_sqr(entity.position())
    }

    pub fn distance_to(&self, entity: &Entity) -> f32 {
        let xd = (self.x() - entity.x()) as f32;
        let yd = (self.y() - entity.y()) as f32;
        let zd = (self.z() - entity.z()) as f32;
        mth::sqrt(xd * xd + yd * yd + zd * zd)
    }

    pub fn is_sprinting(&self) -> bool {
        self.shared_flag(flags::SPRINTING)
    }

    pub fn is_swimming(&self) -> bool {
        self.shared_flag(flags::SWIMMING)
    }

    pub fn set_swimming(&mut self, value: bool) {
        self.set_shared_flag(flags::SWIMMING, value);
    }

    pub fn is_fall_flying(&self) -> bool {
        self.shared_flag(flags::FALL_FLYING)
    }

    pub fn pose(&self) -> Pose {
        match self.data.get(ids::POSE) {
            DataValue::Pose(p) => *p,
            _ => Pose::Standing,
        }
    }

    pub fn set_pose(&mut self, pose: Pose) {
        self.data.set(ids::POSE, DataValue::Pose(pose));
    }

    pub fn is_crouching(&self) -> bool {
        self.pose() == Pose::Crouching
    }

    pub fn is_silent(&self) -> bool {
        matches!(self.data.get(ids::SILENT), DataValue::Boolean(true))
    }

    pub fn is_no_gravity(&self) -> bool {
        matches!(self.data.get(ids::NO_GRAVITY), DataValue::Boolean(true))
    }

    pub fn is_invisible(&self) -> bool {
        self.shared_flag(flags::INVISIBLE)
    }

    pub fn set_no_gravity(&mut self, no_gravity: bool) {
        self.data.set(ids::NO_GRAVITY, DataValue::Boolean(no_gravity));
    }

    pub fn ticks_frozen(&self) -> i32 {
        match self.data.get(ids::TICKS_FROZEN) {
            DataValue::Int(v) => *v,
            _ => 0,
        }
    }

    pub fn set_ticks_frozen(&mut self, ticks: i32) {
        self.data.set(ids::TICKS_FROZEN, DataValue::Int(ticks));
    }

    pub fn air_supply(&self) -> i32 {
        match self.data.get(ids::AIR_SUPPLY) {
            DataValue::Int(v) => *v,
            _ => 300,
        }
    }

    pub fn set_air_supply(&mut self, supply: i32) {
        self.data.set(ids::AIR_SUPPLY, DataValue::Int(supply));
    }

    pub fn get_max_air_supply(&self) -> i32 {
        if self.entity_type == super::entity_types::AXOLOTL.id {
            super::animal::axolotl::axolotl::AXOLOTL_TOTAL_AIR_SUPPLY
        } else if self.entity_type == super::entity_types::DOLPHIN.id {
            super::animal::dolphin::dolphin::TOTAL_AIR_SUPPLY
        } else {
            TOTAL_AIR_SUPPLY
        }
    }

    pub fn is_freezing(&self) -> bool {
        self.ticks_frozen() > 0
    }

    pub fn is_in_water(&self) -> bool {
        self.was_touching_water
    }

    pub fn is_in_lava(&self) -> bool {
        !self.first_tick && self.fluid_interaction.is_in_fluid(&wafer_data::fluid_tags::LAVA)
    }

    pub fn is_under_water(&self) -> bool {
        self.was_eye_in_water && self.is_in_water()
    }

    pub fn is_in_liquid(&self) -> bool {
        self.is_in_water() || self.is_in_lava()
    }

    pub fn fluid_height(&self, fluid: &wafer_data::Tag) -> f64 {
        self.fluid_interaction.fluid_height(fluid)
    }

    pub fn is_eye_in_fluid(&self, fluid: &wafer_data::Tag) -> bool {
        self.fluid_interaction.is_eye_in_fluid(fluid)
    }

    pub fn fluid_jump_threshold(&self) -> f64 {
        if self.entity_type == super::entity_types::SULFUR_CUBE.id {
            return self.dimensions.height as f64 * 0.2;
        }
        if self.entity_type == super::entity_types::BREEZE.id {
            return self.dimensions.eye_height as f64;
        }
        if (self.dimensions.eye_height as f64) < 0.4 { 0.0 } else { 0.4 }
    }

    pub fn reset_fall_distance(&mut self) {
        self.fall_distance = 0.0;
    }

    pub fn check_fall_distance_accumulation(&mut self) {
        if self.delta_movement.y > -0.5 && self.fall_distance > 1.0 {
            self.fall_distance = 1.0;
        }
    }

    pub fn compute_speed(&mut self) {
        let last = *self.last_known_position.get_or_insert(self.pos);
        self.last_known_speed = self.pos.subtract(last);
        self.last_known_position = Some(self.pos);
    }

    pub fn has_moved_horizontally_recently(&self) -> bool {
        self.last_known_speed.horizontal_distance().abs() > 1.0e-5f32 as f64
    }

    pub fn add_movement_this_tick(&mut self, movement: Movement) {
        if self.movement_this_tick.len() >= 100 {
            let first = self.movement_this_tick.pop_front().expect("movement");
            let second = self.movement_this_tick.pop_front().expect("movement");
            self.movement_this_tick.push_front(Movement { from: first.from, to: second.to, axis_dependent_original_movement: None });
        }
        self.movement_this_tick.push_back(movement);
    }

    pub fn limit_piston_movement(&mut self, vec: Vec3, current_game_time: i64) -> Vec3 {
        if vec.length_sqr() <= 1.0e-7 {
            return vec;
        }
        if current_game_time != self.piston_deltas_game_time {
            self.piston_deltas = [0.0; 3];
            self.piston_deltas_game_time = current_game_time;
        }
        if vec.x != 0.0 {
            let xa = self.apply_piston_movement_restriction(Axis::X, vec.x);
            if xa.abs() <= 1.0e-5f32 as f64 { Vec3::ZERO } else { Vec3::new(xa, 0.0, 0.0) }
        } else if vec.y != 0.0 {
            let ya = self.apply_piston_movement_restriction(Axis::Y, vec.y);
            if ya.abs() <= 1.0e-5f32 as f64 { Vec3::ZERO } else { Vec3::new(0.0, ya, 0.0) }
        } else if vec.z != 0.0 {
            let za = self.apply_piston_movement_restriction(Axis::Z, vec.z);
            if za.abs() <= 1.0e-5f32 as f64 { Vec3::ZERO } else { Vec3::new(0.0, 0.0, za) }
        } else {
            Vec3::ZERO
        }
    }

    fn apply_piston_movement_restriction(&mut self, axis: Axis, amount: f64) -> f64 {
        let ordinal = axis as usize;
        let min = mth::clamp(amount + self.piston_deltas[ordinal], -0.51, 0.51);
        let amount = min - self.piston_deltas[ordinal];
        self.piston_deltas[ordinal] = min;
        amount
    }

    pub fn remove_latest_movement_recording(&mut self) {
        self.movement_this_tick.pop_back();
    }

    pub fn clear_movement_this_tick(&mut self) {
        self.movement_this_tick.clear();
    }

    fn on_pos_below(&self, offset: f32) -> BlockPos {
        BlockPos::new(mth::floor(self.pos.x), mth::floor(self.pos.y - offset as f64), mth::floor(self.pos.z))
    }

    pub fn move_relative(&mut self, speed: f32, input: Vec3) {
        let delta = input_vector(input, speed, self.y_rot);
        self.set_delta_movement(self.delta_movement.add_vec(delta));
    }

    pub fn clear_fire(&mut self) {
        self.remaining_fire_ticks = self.remaining_fire_ticks.min(0);
    }

    pub fn ignite_for_ticks(&mut self, ticks: i32) {
        if self.remaining_fire_ticks < ticks {
            self.remaining_fire_ticks = ticks;
        }
        self.set_ticks_frozen(0);
    }
}

pub fn input_vector(input: Vec3, speed: f32, y_rot: f32) -> Vec3 {
    let length = input.length_sqr();
    if length < 1.0e-7 {
        return Vec3::ZERO;
    }
    let movement = if length > 1.0 { input.normalize() } else { input }.scale(speed as f64);
    let sin = mth::sin((y_rot * mth::DEG_TO_RAD) as f64) as f64;
    let cos = mth::cos((y_rot * mth::DEG_TO_RAD) as f64) as f64;
    Vec3::new(movement.x * cos - movement.z * sin, movement.y, movement.z * cos + movement.x * sin)
}

pub fn create_insecure_uuid(random: &mut impl RandomSource) -> u128 {
    let most = random.next_long() & -61441 | 16384;
    let least = random.next_long() & 4611686018427387903 | i64::MIN;
    (most as u64 as u128) << 64 | least as u64 as u128
}

const ON_POS_LEGACY: f32 = 0.2;
const BELOW_AFFECTS_MOVEMENT: f32 = 0.500001;
const ITEM_BELOW_AFFECTS_MOVEMENT: f32 = 0.999999;
const ON_POS: f32 = 1.0e-5;

pub fn sound_event(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

pub fn calculate_view_quaternion(x_rot: f32, y_rot: f32) -> wafer_core::joml::Quaternionf {
    wafer_core::joml::Quaternionf::rotation_y(-y_rot * mth::DEG_TO_RAD).rotate_x(x_rot * mth::DEG_TO_RAD)
}

pub fn calculate_view_vector(x_rot: f32, y_rot: f32) -> Vec3 {
    let real_x_rot = x_rot * mth::DEG_TO_RAD;
    let real_y_rot = -y_rot * mth::DEG_TO_RAD;
    let y_cos = mth::cos(real_y_rot as f64);
    let y_sin = mth::sin(real_y_rot as f64);
    let x_cos = mth::cos(real_x_rot as f64);
    let x_sin = mth::sin(real_x_rot as f64);
    Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
}

pub fn compute_modified_friction(friction: f32, modifier: f32) -> f32 {
    mth::clamp_f(1.0 - (1.0 - friction) * modifier, 0.0, 1.0)
}

impl ServerLevel {
    pub fn entity(&self, id: EntityId) -> &Entity {
        self.entity_ref(id).expect("entity in this level")
    }

    pub fn entity_mut(&mut self, id: EntityId) -> &mut Entity {
        if self.players.contains_key(&id) {
            return &mut self.players.get_mut(&id).expect("a player").entity;
        }
        if self.entities.contains_key(&id) {
            return self.entities.get_mut(&id).expect("entity in this level").entity_mut();
        }
        self.dragon_part_entity_mut(id).expect("entity in this level")
    }

    pub fn is_player(&self, id: EntityId) -> bool {
        self.players.contains_key(&id)
    }

    pub fn get_loot_table(&self, id: EntityId) -> Option<String> {
        if let Some(mob) = self.mob(id) {
            return mob.loot_table_id();
        }
        self.is_player(id).then(|| "minecraft:entities/player".to_string())
    }

    pub fn force_set_rotation(&mut self, id: EntityId, y_rot: f32, relative_y: bool, x_rot: f32, relative_x: bool) {
        let living = self.is_living(id);
        let e = self.entity_mut(id);
        let absolute_y = if relative_y { e.y_rot } else { 0.0 } + y_rot;
        let absolute_x = mth::clamp_f(if relative_x { e.x_rot } else { 0.0 } + x_rot, -90.0, 90.0);
        e.set_y_rot(absolute_y);
        if living {
            e.y_head_rot = absolute_y;
        }
        e.set_x_rot(absolute_x);
        if let Some(player) = self.players.get(&id) {
            player.connection.send(wafer_data::packets::play::clientbound::PLAYER_ROTATION, |w| {
                w.f32(y_rot);
                w.bool(relative_y);
                w.f32(x_rot);
                w.bool(relative_x);
            });
        }
    }

    pub fn attribute_value(&self, id: EntityId, attribute: Attribute) -> f64 {
        self.living(id).attributes.value(attribute)
    }

    pub fn is_spectator(&self, id: EntityId) -> bool {
        self.players.get(&id).is_some_and(|p| p.game_mode.game_type == GameType::Spectator)
    }

    pub fn can_be_hit_by_projectile(&self, id: EntityId) -> bool {
        self.is_alive(id) && self.is_pickable(id)
    }

    pub fn can_be_picked(&self, id: EntityId) -> bool {
        !self.is_spectator(id) && self.is_pickable(id)
    }

    pub fn add_entity_data(&self, id: EntityId) -> i32 {
        match self.entities.get(&id) {
            Some(LevelEntity::Projectile(_)) => self.projectile_add_data(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Warden(_)) => (mob.entity.pose() == super::data::Pose::Emerging) as i32,
            Some(entity) => entity.add_entity_data(),
            None => 0,
        }
    }

    pub fn extinguish_fire(&mut self, id: EntityId) {
        if self.is_on_fire(id) {
            self.play_entity_on_fire_extinguished_sound(id);
        }
        self.clear_fire(id);
    }

    pub fn entity_look_at(&mut self, id: EntityId, anchor: crate::commands::source::Anchor, pos: Vec3) {
        let from = crate::server::commands::look_at::anchor_apply(self, anchor, id);
        let (xd, yd, zd) = (pos.x - from.x, pos.y - from.y, pos.z - from.z);
        let sd = (xd * xd + zd * zd).sqrt();
        let living = self.is_living(id);
        let e = self.entity_mut(id);
        e.set_x_rot(mth::wrap_degrees((-(mth::atan2(yd, sd) * mth::RAD_TO_DEG as f64)) as f32));
        e.set_y_rot(mth::wrap_degrees((mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0));
        if living {
            e.y_head_rot = e.y_rot;
        }
        e.x_rot_o = e.x_rot;
        e.y_rot_o = e.y_rot;
        if living {
            let y_head_rot = e.y_head_rot;
            let living = self.living_mut(id);
            living.y_head_rot_o = y_head_rot;
            living.y_body_rot = y_head_rot;
            living.y_body_rot_o = living.y_body_rot;
        }
    }

    pub fn known_speed(&self, id: EntityId) -> Vec3 {
        if let Some(player) = self.players.get(&id) {
            return match self.vehicle(id) {
                Some(vehicle) if self.controlling_passenger(vehicle) != Some(id) => self.known_speed(vehicle),
                _ => player.known_movement,
            };
        }
        match self.controlling_passenger(id).filter(|&c| self.is_player(c)) {
            Some(controller) if self.is_alive(id) => self.known_speed(controller),
            _ => self.entity(id).last_known_speed,
        }
    }

    pub fn is_invulnerable_to_piercing_weapon(&self, id: EntityId) -> bool {
        if self.mob(id).is_some_and(|mob| mob.sulfur_cube().is_some()) {
            return self.entity(id).is_invulnerable() && !self.mob(id).expect("a sulfur cube").sulfur_cube_is_primed();
        }
        self.entity(id).is_invulnerable()
    }

    pub fn known_movement(&self, id: EntityId) -> Vec3 {
        if let Some(player) = self.players.get(&id) {
            return match self.vehicle(id) {
                Some(vehicle) if self.controlling_passenger(vehicle) != Some(id) => self.known_movement(vehicle),
                _ => player.known_movement,
            };
        }
        let known_movement = match self.controlling_passenger(id).filter(|&c| self.is_player(c)) {
            Some(controller) if self.is_alive(id) => self.known_movement(controller),
            _ => self.entity(id).delta_movement,
        };
        if self.is_minecart(id) { ServerLevel::old_minecart_behavior_known_movement(known_movement) } else { known_movement }
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        match self.players.get(&id) {
            Some(p) => !p.entity.is_removed() && p.health > 0.0,
            None => match self.mob(id) {
                Some(mob) => !mob.entity.is_removed() && mob.health > 0.0,
                None => self.entity_ref(id).is_some_and(|e| !e.is_removed()),
            },
        }
    }

    pub fn fire_immune(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(super::LevelEntity::Item(item)) => !item.item.can_be_hurt_by(&crate::world::damagesource::DamageSource::new(wafer_data::damage_types::IN_FIRE)),
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Creaking(_)) && super::monster::creaking::creaking::is_heart_bound(&mob.entity) => true,
            _ => self.entity_ref(id).is_some_and(|e| super::entity_type::fire_immune(e.entity_type)),
        }
    }

    pub fn is_on_fire(&self, id: EntityId) -> bool {
        if self.is_shulker_bullet(id) || self.is_wither_skull(id) {
            return false;
        }
        match self.mob(id) {
            Some(mob) if matches!(mob.kind, super::mob::MobKind::Blaze(_)) => super::monster::blaze::Blaze::is_on_fire(&mob.entity),
            Some(mob) if matches!(mob.kind, super::mob::MobKind::Strider(_)) => false,
            _ => self.entity_is_on_fire(id),
        }
    }

    pub fn entity_is_on_fire(&self, id: EntityId) -> bool {
        !self.fire_immune(id) && self.entity(id).remaining_fire_ticks > 0
    }

    pub fn get_entity_light_level_dependent_magic_value(&self, id: EntityId) -> f32 {
        if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::MagmaCube(_) | super::mob::MobKind::SulfurCube(_))) {
            return super::monster::cubemob::magma_cube::MagmaCube::get_light_level_dependent_magic_value();
        }
        if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Blaze(_))) {
            return super::monster::blaze::Blaze::get_light_level_dependent_magic_value();
        }
        if self.is_vex(id) {
            return super::monster::vex::Vex::get_light_level_dependent_magic_value();
        }
        let e = self.entity(id);
        let block_pos = e.block_position();
        if self.chunks.loaded_chunk(block_pos.chunk()).is_some() {
            self.get_light_level_dependent_magic_value(BlockPos::containing(e.x(), e.eye_y(), e.z()))
        } else {
            0.0
        }
    }

    fn fire_immune_ticks(&self, id: EntityId) -> i32 {
        if self.is_player(id) { 20 } else { 0 }
    }

    pub fn ignite_for_seconds(&mut self, id: EntityId, number_of_seconds: f32) {
        self.ignite_for_ticks(id, mth::floor_f(number_of_seconds * 20.0));
    }

    pub fn ignite_for_ticks(&mut self, id: EntityId, ticks: i32) {
        let ticks = if self.is_living(id) { mth::ceil(ticks as f64 * self.attribute_value(id, Attribute::BurningTime)) } else { ticks };
        if self.entity(id).remaining_fire_ticks < ticks {
            self.set_remaining_fire_ticks(id, ticks);
        }
        self.entity_mut(id).set_ticks_frozen(0);
    }

    pub fn max_up_step(&self, id: EntityId) -> f32 {
        if let Some(step) = self.mob(id).and_then(|mob| mob.sulfur_cube().and(mob.sulfur_cube_max_up_step())) {
            return step;
        }
        if self.is_living(id) { self.attribute_value(id, Attribute::StepHeight) as f32 } else { 0.0 }
    }

    fn default_gravity(&self, id: EntityId) -> f64 {
        use super::projectile::projectile::ProjectileKind;
        match self.entities.get(&id) {
            Some(super::LevelEntity::Mob(mob)) if mob.is_squid() => super::animal::squid::squid::Squid::get_default_gravity(),
            None | Some(super::LevelEntity::Mob(_) | super::LevelEntity::ArmorStand(_) | super::LevelEntity::Mannequin(_)) => self.attribute_value(id, Attribute::Gravity),
            Some(super::LevelEntity::ExperienceOrb(_)) => 0.03,
            Some(super::LevelEntity::Projectile(projectile)) => match projectile.kind {
                ProjectileKind::ThrownExperienceBottle(_) => 0.07,
                ProjectileKind::ThrownSplashPotion(_)
                | ProjectileKind::ThrownLingeringPotion(_)
                | ProjectileKind::Arrow(_)
                | ProjectileKind::SpectralArrow(_)
                | ProjectileKind::ThrownTrident(_) => 0.05,
                ProjectileKind::FireworkRocket(_) => 0.0,
                ProjectileKind::SmallFireball(_) | ProjectileKind::LargeFireball(_) | ProjectileKind::WindCharge(_) | ProjectileKind::BreezeWindCharge(_) | ProjectileKind::WitherSkull(_) | ProjectileKind::DragonFireball(_) => 0.0,
                ProjectileKind::FishingHook(_) => super::projectile::fishing_hook::GRAVITY as f64,
                ProjectileKind::ShulkerBullet(_) => super::projectile::shulker_bullet::GRAVITY,
                ProjectileKind::LlamaSpit(_) => 0.06,
                _ => 0.03,
            },
            Some(super::LevelEntity::AreaEffectCloud(_)) => 0.0,
            Some(super::LevelEntity::Minecart(_)) => self.abstract_minecart_default_gravity(id),
            Some(_) => 0.04,
        }
    }

    pub fn gravity(&self, id: EntityId) -> f64 {
        if self.entity(id).is_no_gravity() { 0.0 } else { self.default_gravity(id) }
    }

    pub fn effective_gravity(&self, id: EntityId) -> f64 {
        let is_falling = self.entity(id).delta_movement.y <= 0.0;
        if is_falling && self.is_living(id) && self.has_effect(id, wafer_data::mob_effects::SLOW_FALLING) { self.gravity(id).min(0.01) } else { self.gravity(id) }
    }

    pub fn apply_gravity(&mut self, id: EntityId) {
        let gravity = self.gravity(id);
        if gravity != 0.0 {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -gravity, 0.0));
        }
    }

    pub fn air_drag(&self, id: EntityId) -> f32 {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_)) => 1.0,
            Some(LevelEntity::Minecart(_)) => 0.95,
            Some(LevelEntity::Projectile(projectile)) if !matches!(projectile.kind, super::projectile::projectile::ProjectileKind::FireworkRocket(_)) => 0.99,
            _ if self.is_living(id) => compute_modified_friction(if self.omnidirectional_air_mover(id) { 0.91 } else { 0.98 }, self.attribute_value(id, Attribute::AirDragModifier) as f32),
            _ => 0.98,
        }
    }

    fn entity_bounciness(&self, id: EntityId) -> f64 {
        if self.is_living(id) { self.attribute_value(id, Attribute::Bounciness) } else { 0.0 }
    }

    fn movement_emission(&self, id: EntityId) -> MovementEmission {
        if let Some(player) = self.players.get(&id) {
            return if player.abilities_flying() || player.entity.on_ground && player.entity.is_discrete() { MovementEmission::None } else { MovementEmission::All };
        }
        match self.entities.get(&id) {
            Some(super::LevelEntity::ExperienceOrb(_) | super::LevelEntity::FallingBlock(_) | super::LevelEntity::Item(_) | super::LevelEntity::PrimedTnt(_) | super::LevelEntity::EndCrystal(_)) => MovementEmission::None,
            Some(super::LevelEntity::Projectile(projectile)) if projectile.abstract_arrow().is_some() || matches!(projectile.kind, super::projectile::projectile::ProjectileKind::FishingHook(_)) => MovementEmission::None,
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Silverfish(_) | super::mob::MobKind::Endermite(_) | super::mob::MobKind::Breeze(_)) => MovementEmission::Events,
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Bat(_)) || mob.is_squid() => MovementEmission::Events,
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Guardian(_) | super::mob::MobKind::ElderGuardian(_)) => MovementEmission::Events,
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Shulker(_)) => MovementEmission::None,
            Some(super::LevelEntity::Boat(_) | super::LevelEntity::Minecart(_)) => MovementEmission::Events,
            _ => MovementEmission::All,
        }
    }

    pub fn is_pushed_by_fluid(&self, id: EntityId) -> bool {
        match self.players.get(&id) {
            Some(p) => !p.abilities_flying(),
            None => match self.as_projectile(id) {
                Some(projectile) if projectile.abstract_arrow().is_some() => self.abstract_arrow_is_pushed_by_fluid(id),
                _ if self.is_drowned(id) => super::monster::zombie::drowned::Drowned::is_pushed_by_fluid(self.mob(id).expect("a drowned")),
                _ if self.mob(id).is_some_and(|mob| mob.is_squid() || matches!(mob.kind, super::mob::MobKind::Dolphin(_))) => super::animal::ageable_water_creature::is_pushed_by_fluid(),
                _ if self.mob(id).is_some_and(super::mob::Mob::is_water_animal) => super::animal::fish::water_animal::is_pushed_by_fluid(),
                _ if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Axolotl(_))) => super::animal::axolotl::axolotl::is_pushed_by_fluid(),
                _ if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Frog(_))) => super::animal::frog::frog::is_pushed_by_fluid(),
                _ if self.is_turtle(id) => super::animal::turtle::turtle::Turtle::is_pushed_by_fluid(),
                _ if self.is_abstract_nautilus(id) => super::animal::nautilus::abstract_nautilus::is_pushed_by_fluid(),
                _ => true,
            },
        }
    }

    pub fn is_local_instance_authoritative(&self, id: EntityId) -> bool {
        !self.is_client_authoritative(id)
    }

    pub fn is_client_authoritative(&self, id: EntityId) -> bool {
        if self.is_player(id) {
            return true;
        }
        self.controlling_passenger(id).is_some_and(|passenger| self.is_client_authoritative(passenger))
    }

    pub fn can_simulate_movement(&self, id: EntityId) -> bool {
        if self.is_player(id) {
            return true;
        }
        match self.entities.get(&id) {
            Some(LevelEntity::ExperienceOrb(_) | LevelEntity::FallingBlock(_)) => true,
            _ => self.is_local_instance_authoritative(id),
        }
    }

    pub fn sound_source(&self, id: EntityId) -> SoundSource {
        match self.entities.get(&id) {
            None => SoundSource::Players,
            Some(super::LevelEntity::ExperienceOrb(_)) => SoundSource::Ambient,
            Some(_) if self.is_monster(id) => self.monster_get_sound_source(),
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Hoglin(_) | super::mob::MobKind::Shulker(_) | super::mob::MobKind::EnderDragon(_)) => SoundSource::Hostile,
            Some(super::LevelEntity::Projectile(projectile)) if matches!(projectile.kind, super::projectile::projectile::ProjectileKind::ShulkerBullet(_)) => SoundSource::Hostile,
            Some(super::LevelEntity::Mob(mob)) if mob.sulfur_cube().is_some() => SoundSource::Neutral,
            Some(super::LevelEntity::Mob(mob)) if mob.is_cube_mob() => self.abstract_cube_mob_get_sound_source(),
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Ghast(_)) => super::monster::ghast::Ghast::get_sound_source(),
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Phantom(_)) => super::monster::phantom::Phantom::get_sound_source(),
            Some(super::LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Rabbit(_)) => super::animal::rabbit::rabbit::Rabbit::get_sound_source(mob),
            Some(_) => SoundSource::Neutral,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn play_sound_at(&mut self, except: Option<EntityId>, x: f64, y: f64, z: f64, sound: impl Into<crate::sounds::sound_event::SoundEvent>, source: SoundSource, volume: f32, pitch: f32) {
        let seed = self.thread_safe_random.next_long();
        self.play_seeded_sound_at(except, x, y, z, sound, source, volume, pitch, seed);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn play_seeded_sound_at(&self, except: Option<EntityId>, x: f64, y: f64, z: f64, sound: impl Into<crate::sounds::sound_event::SoundEvent>, source: SoundSource, volume: f32, pitch: f32, seed: i64) {
        use crate::network::connection::encode;
        use wafer_data::packets::play::clientbound;
        let sound = sound.into();
        let packet = encode(clientbound::SOUND, |p| {
            sound.write(p);
            p.varint(source as i32);
            p.i32((x * 8.0) as i32);
            p.i32((y * 8.0) as i32);
            p.i32((z * 8.0) as i32);
            p.f32(volume);
            p.f32(pitch);
            p.i64(seed);
        });
        let range = sound.get_range(volume) as f64;
        self.broadcast_near(except, x, y, z, range, &packet);
    }

    pub fn play_sound_from_entity(&mut self, except: Option<EntityId>, source_entity: EntityId, sound: impl Into<crate::sounds::sound_event::SoundEvent>, source: SoundSource, volume: f32, pitch: f32) {
        let seed = self.thread_safe_random.next_long();
        self.play_seeded_sound_from_entity(except, source_entity, sound, source, volume, pitch, seed);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn play_seeded_sound_from_entity(&self, except: Option<EntityId>, source_entity: EntityId, sound: impl Into<crate::sounds::sound_event::SoundEvent>, source: SoundSource, volume: f32, pitch: f32, seed: i64) {
        use crate::network::connection::encode;
        use wafer_data::packets::play::clientbound;
        let sound = sound.into();
        let packet = encode(clientbound::SOUND_ENTITY, |p| {
            sound.write(p);
            p.varint(source as i32);
            p.varint(source_entity);
            p.f32(volume);
            p.f32(pitch);
            p.i64(seed);
        });
        let e = self.entity(source_entity);
        let range = sound.get_range(volume) as f64;
        self.broadcast_near(except.filter(|&except| self.is_player(except)), e.x(), e.y(), e.z(), range, &packet);
    }

    pub fn entity_play_sound(&mut self, id: EntityId, sound: impl Into<crate::sounds::sound_event::SoundEvent>, volume: f32, pitch: f32) {
        let source = self.sound_source(id);
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        if self.is_player(id) {
            self.play_sound_at(Some(id), x, y, z, sound, source, volume, pitch);
        } else if !e.is_silent() {
            self.play_sound_at(None, x, y, z, sound, source, volume, pitch);
        }
    }

    pub fn on_pos(&mut self, id: EntityId, offset: f32) -> BlockPos {
        let e = self.entity(id);
        let Some(on_pos) = e.main_supporting_block_pos else { return e.on_pos_below(offset) };
        if !(offset > 1.0e-5) {
            return on_pos;
        }
        let y = mth::floor(e.y() - offset as f64);
        let below = self.block_state(on_pos);
        if (!(offset as f64 <= 0.5) || !below.is(&block_tags::FENCES)) && !below.is(&block_tags::WALLS) && !below.block().is_instance_of("FenceGateBlock") {
            BlockPos::new(on_pos.x, y, on_pos.z)
        } else {
            on_pos
        }
    }

    pub fn on_pos_legacy(&mut self, id: EntityId) -> BlockPos {
        self.on_pos(id, ON_POS_LEGACY)
    }

    pub fn block_pos_below_that_affects_my_movement(&mut self, id: EntityId) -> BlockPos {
        let offset = if self.is_living(id) { BELOW_AFFECTS_MOVEMENT } else { ITEM_BELOW_AFFECTS_MOVEMENT };
        self.on_pos(id, offset)
    }

    pub fn set_on_ground(&mut self, id: EntityId, on_ground: bool) {
        self.entity_mut(id).on_ground = on_ground;
        self.check_supporting_block(id, on_ground, None);
    }

    pub fn set_on_ground_with_movement(&mut self, id: EntityId, on_ground: bool, horizontal_collision: bool, movement: Vec3) {
        let e = self.entity_mut(id);
        e.on_ground = on_ground;
        e.horizontal_collision = horizontal_collision;
        self.check_supporting_block(id, on_ground, Some(movement));
    }

    fn check_supporting_block(&mut self, id: EntityId, on_ground: bool, movement: Option<Vec3>) {
        let e = self.entity(id);
        if on_ground {
            let bb = e.bounding_box();
            let test_area = AABB::new(bb.min_x, bb.min_y - 1.0e-6, bb.min_z, bb.max_x, bb.min_y, bb.max_z);
            let position = e.position();
            let no_blocks = e.on_ground_no_blocks;
            let mut supporting = self.find_supporting_block(id, position, &test_area);
            if supporting.is_some() || no_blocks {
                self.entity_mut(id).main_supporting_block_pos = supporting;
            } else if let Some(movement) = movement {
                supporting = self.find_supporting_block(id, position, &test_area.move_by(-movement.x, 0.0, -movement.z));
                self.entity_mut(id).main_supporting_block_pos = supporting;
            }
            self.entity_mut(id).on_ground_no_blocks = supporting.is_none();
        } else {
            let e = self.entity_mut(id);
            e.on_ground_no_blocks = false;
            e.main_supporting_block_pos = None;
        }
    }

    pub fn block_speed_factor(&mut self, id: EntityId) -> f32 {
        if self.is_minecart(id)
            && let Some(factor) = self.abstract_minecart_block_speed_factor(id)
        {
            return factor;
        }
        if self.players.get(&id).is_some_and(|p| p.abilities_flying() || p.entity.is_fall_flying()) {
            return 1.0;
        }
        let state = self.block_state(self.entity(id).block_position());
        let here = state.block().info().speed_factor;
        let factor = if !state.is_block(blocks::WATER) && !state.is_block(blocks::BUBBLE_COLUMN) {
            if here == 1.0 {
                let below = self.block_pos_below_that_affects_my_movement(id);
                self.block_state(below).block().info().speed_factor
            } else {
                here
            }
        } else {
            here
        };
        if self.is_living(id) { mth::lerp_f(self.attribute_value(id, Attribute::MovementEfficiency) as f32, factor, 1.0) } else { factor }
    }

    pub fn block_jump_factor(&mut self, id: EntityId) -> f32 {
        let here = self.block_state(self.entity(id).block_position()).block().info().jump_factor;
        let below_pos = self.block_pos_below_that_affects_my_movement(id);
        let below = self.block_state(below_pos).block().info().jump_factor;
        if here == 1.0 { below } else { here }
    }

    pub fn in_block_state(&mut self, id: EntityId) -> BlockState {
        if let Some(state) = self.entity(id).in_block_state {
            return state;
        }
        let state = self.block_state(self.entity(id).block_position());
        self.entity_mut(id).in_block_state = Some(state);
        state
    }

    pub fn move_entity(&mut self, id: EntityId, mover: MoverType, delta: Vec3) {
        if self.is_minecart(id) {
            self.minecart_move(id, mover, delta);
        } else if self.is_shulker(id) {
            self.shulker_move(id, mover, delta);
        } else {
            self.entity_move(id, mover, delta);
        }
        if self.as_projectile(id).is_some_and(|projectile| projectile.abstract_arrow().is_some()) {
            self.abstract_arrow_after_move(id, mover);
        }
    }

    pub fn entity_move(&mut self, id: EntityId, mover: MoverType, delta: Vec3) {
        if self.is_block_attached_entity(id) {
            self.block_attached_entity_move(id, delta);
            return;
        }
        let e = self.entity_mut(id);
        if e.no_physics {
            let to = e.position().add_vec(delta);
            e.set_pos(to);
            self.entity_moved(id);
            self.record_movement(id, mover, delta);
            let e = self.entity_mut(id);
            e.horizontal_collision = false;
            e.vertical_collision = false;
            e.vertical_collision_below = false;
            e.minor_horizontal_collision = false;
            return;
        }
        let mut delta = delta;
        if mover == MoverType::Piston {
            let game_time = self.game_time;
            delta = self.entity_mut(id).limit_piston_movement(delta, game_time);
            if delta == Vec3::ZERO {
                return;
            }
        }
        let e = self.entity_mut(id);
        if e.stuck_speed_multiplier.length_sqr() > 1.0e-7 {
            if mover != MoverType::Piston {
                let m = e.stuck_speed_multiplier;
                delta = delta.multiply(m.x, m.y, m.z);
            }
            e.stuck_speed_multiplier = Vec3::ZERO;
            e.set_delta_movement(Vec3::ZERO);
        }
        let delta = self.maybe_back_off_from_edge(id, delta, mover);
        let movement = self.collide(id, delta);
        let movement_length = movement.length_sqr();
        if movement_length > 1.0e-7 || delta.length_sqr() - movement_length < 1.0e-7 {
            let e = self.entity(id);
            if e.fall_distance != 0.0 && movement_length >= 1.0 {
                let check_distance = movement.length().min(8.0);
                let check_to = e.position().add_vec(movement.normalize().scale(check_distance));
                let ctx = ClipContext {
                    from: e.position(),
                    to: check_to,
                    block: ClipBlock::FallDamageResetting,
                    fluid: ClipFluid::Water,
                    is_player: self.is_player(id),
                };
                if !self.clip(&ctx).miss {
                    self.reset_fall_distance(id);
                }
            }
            let e = self.entity_mut(id);
            let pos = e.position();
            let new_position = pos.add_vec(movement);
            e.add_movement_this_tick(Movement { from: pos, to: new_position, axis_dependent_original_movement: Some(delta) });
            e.set_pos(new_position);
            self.entity_moved(id);
            self.record_movement(id, mover, movement);
        }
        let x_collision = !mth::equal(delta.x, movement.x);
        let z_collision = !mth::equal(delta.z, movement.z);
        let local = self.is_local_instance_authoritative(id);
        let e = self.entity_mut(id);
        e.horizontal_collision = x_collision || z_collision;
        let moved_vertically = delta.y.abs() > 0.0;
        if moved_vertically || local {
            e.vertical_collision = delta.y != movement.y;
            e.vertical_collision_below = e.vertical_collision && delta.y < 0.0;
            let (below, horizontal) = (e.vertical_collision_below, e.horizontal_collision);
            self.set_on_ground_with_movement(id, below, horizontal, movement);
        }
        let e = self.entity_mut(id);
        e.minor_horizontal_collision = false;
        let effect_pos = self.on_pos_legacy(id);
        let effect_state = self.block_state(effect_pos);
        if local {
            let on_ground = self.entity(id).on_ground;
            self.check_fall_damage(id, movement.y, on_ground, effect_state, effect_pos);
        }
        if self.entity_ref(id).is_none_or(Entity::is_removed) {
            return;
        }
        let e = self.entity(id);
        if self.can_simulate_movement(id) && (moved_vertically && e.vertical_collision || e.horizontal_collision) {
            self.restitute_movement_after_collisions(id, effect_state, x_collision, z_collision, movement);
        }
        let emission = self.movement_emission(id);
        if emission.emits_anything() && !self.is_passenger(id) {
            self.apply_movement_emission_and_play_sound(id, emission, movement, effect_pos, effect_state);
        }
        let factor = self.block_speed_factor(id) as f64;
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.multiply(factor, 1.0, factor));
    }

    fn record_movement(&mut self, id: EntityId, mover: MoverType, movement: Vec3) {
        if mover.is_server_and_client_simulated()
            && let Some(tracker) = self.chunk_map.tracked.get_mut(id).and_then(|t| t.server_entity.interpolation())
        {
            tracker.apply_predicted_movement(movement);
        }
    }

    fn maybe_back_off_from_edge(&mut self, id: EntityId, delta: Vec3, mover: MoverType) -> Vec3 {
        let Some(p) = self.players.get(&id) else { return delta };
        let max_down_step = self.max_up_step(id);
        let flying = p.abilities_flying();
        let shift = p.entity.is_shift_key_down();
        if flying || delta.y > 0.0 || !matches!(mover, MoverType::SelfMove | MoverType::Player) || !shift || !self.is_above_ground(id, max_down_step) {
            return delta;
        }
        let (mut dx, mut dz) = (delta.x, delta.z);
        let step = 0.05;
        let step_x = dx.signum_java() * step;
        let step_z = dz.signum_java() * step;
        while dx != 0.0 && self.can_fall_at_least(id, dx, 0.0, max_down_step as f64) {
            if dx.abs() <= step {
                dx = 0.0;
                break;
            }
            dx -= step_x;
        }
        while dz != 0.0 && self.can_fall_at_least(id, 0.0, dz, max_down_step as f64) {
            if dz.abs() <= step {
                dz = 0.0;
                break;
            }
            dz -= step_z;
        }
        while dx != 0.0 && dz != 0.0 && self.can_fall_at_least(id, dx, dz, max_down_step as f64) {
            if dx.abs() <= step {
                dx = 0.0;
            } else {
                dx -= step_x;
            }
            if dz.abs() <= step {
                dz = 0.0;
            } else {
                dz -= step_z;
            }
        }
        Vec3::new(dx, delta.y, dz)
    }

    fn is_above_ground(&self, id: EntityId, max_down_step: f32) -> bool {
        let e = self.entity(id);
        e.on_ground || e.fall_distance < max_down_step as f64 && !self.can_fall_at_least(id, 0.0, 0.0, max_down_step as f64 - e.fall_distance)
    }

    fn can_fall_at_least(&self, id: EntityId, dx: f64, dz: f64, min_height: f64) -> bool {
        let bb = self.entity(id).bounding_box();
        self.no_collision_for(id, &AABB::new(
            bb.min_x + 1.0e-7 + dx,
            bb.min_y - min_height - 1.0e-7,
            bb.min_z + 1.0e-7 + dz,
            bb.max_x - 1.0e-7 + dx,
            bb.min_y,
            bb.max_z - 1.0e-7 + dz,
        ))
    }

    pub(crate) fn colliders<'a>(&'a self, id: EntityId, entity_colliders: &'a [VoxelShape], box_: &AABB) -> Vec<Collider<'a>> {
        let entities = entity_colliders.iter().map(|shape| Collider::new(shape, [0.0; 3]));
        let border = self.border_collision(id, box_).map(|shape| Collider::new(shape, [0.0; 3]));
        let context = crate::world::phys::shapes::collision_context::CollisionContext::of(self, id);
        entities.chain(border).chain(self.block_collisions(box_, &context, false).into_iter().map(|(_, c)| c)).collect()
    }

    pub fn entity_collisions(&self, source: Option<EntityId>, test_area: &AABB) -> Vec<VoxelShape> {
        if test_area.size() < 1.0e-7 {
            return Vec::new();
        }
        self.get_entities(source, &test_area.inflate_all(1.0e-7))
            .into_iter()
            .filter(|&entity| match source {
                Some(source) => self.can_collide_with(source, entity),
                None => self.can_be_collided_with(entity, None),
            })
            .map(|entity| VoxelShape::from_boxes(&[self.entity(entity).bounding_box()]))
            .collect()
    }

    fn collide(&mut self, id: EntityId, movement: Vec3) -> Vec3 {
        let e = self.entity(id);
        let aabb = e.bounding_box();
        let on_ground = e.on_ground;
        let entity_colliders = self.entity_collisions(Some(id), &aabb.expand_towards_vec(movement).expand_towards(0.0, self.max_up_step(id) as f64, 0.0));
        let movement_step =
            if movement.length_sqr() == 0.0 { movement } else { collide_with_shapes(movement, &aabb, &self.colliders(id, &entity_colliders, &aabb.expand_towards_vec(movement))) };
        let x_collision = movement.x != movement_step.x;
        let y_collision = movement.y != movement_step.y;
        let z_collision = movement.z != movement_step.z;
        let on_ground_after_collision = y_collision && movement.y < 0.0;
        let max_up_step = self.max_up_step(id);
        if max_up_step > 0.0 && (on_ground_after_collision || on_ground) && (x_collision || z_collision) {
            let grounded = if on_ground_after_collision { aabb.move_by(0.0, movement_step.y, 0.0) } else { aabb };
            let mut step_up_aabb = grounded.expand_towards(movement.x, max_up_step as f64, movement.z);
            if !on_ground_after_collision {
                step_up_aabb = step_up_aabb.expand_towards(0.0, -1.0e-5f32 as f64, 0.0);
            }
            let colliders = self.colliders(id, &entity_colliders, &step_up_aabb);
            let step_height_to_skip = movement_step.y as f32;
            for candidate in collect_candidate_step_up_heights(&grounded, &colliders, max_up_step, step_height_to_skip) {
                let step_from_ground = collide_with_shapes(Vec3::new(movement.x, candidate as f64, movement.z), &grounded, &colliders);
                if step_from_ground.horizontal_distance_sqr() > movement_step.horizontal_distance_sqr() {
                    let distance_to_ground = aabb.min_y - grounded.min_y;
                    return step_from_ground.subtract(Vec3::new(0.0, distance_to_ground, 0.0));
                }
            }
        }
        movement_step
    }

    pub fn reset_fall_distance(&mut self, id: EntityId) {
        if let Some(p) = self.players.get(&id) {
            if p.health > 0.0
                && let Some(starting_to_fall_position) = p.starting_to_fall_position
            {
                crate::advancements::triggers::criteria_triggers::FALL_FROM_HEIGHT.trigger(self, id, starting_to_fall_position);
            }
            self.player_mut(id).starting_to_fall_position = None;
        }
        self.entity_mut(id).reset_fall_distance();
    }

    pub fn check_fall_damage(&mut self, id: EntityId, ya: f64, on_ground: bool, on_state: BlockState, pos: BlockPos) {
        if self.is_strider(id) && self.strider_check_fall_damage(id) {
            return;
        }
        if self.is_parrot(id) || self.is_bee(id) || self.is_allay(id) {
            return;
        }
        if self.is_boat(id) {
            self.abstract_boat_check_fall_damage(id, ya, on_ground);
            return;
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Bat(_))) {
            self.bat_check_fall_damage(id);
            return;
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Ghast(_) | super::mob::MobKind::HappyGhast(_))) {
            self.ghast_check_fall_damage(id);
            return;
        }
        if self.is_phantom(id) {
            self.phantom_check_fall_damage(id);
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.spawn_extra_particles_on_fall) && on_ground && self.entity(id).fall_distance > 0.0 {
            let centered = Vec3::at_center_of(pos).add(0.0, 0.5, 0.0);
            let particles = mth::clamp(50.0 * self.entity(id).fall_distance, 0.0, 200.0) as i32;
            self.send_block_particles(on_state, centered.x, centered.y, centered.z, particles, (0.3f32 as f64, 0.3f32 as f64, 0.3f32 as f64), 0.15f32 as f64);
            self.player_mut(id).spawn_extra_particles_on_fall = false;
        }
        let living = self.is_living(id);
        if living && !self.entity(id).is_in_water() {
            self.update_fluid_interaction(id);
        }
        if living && on_ground && self.entity(id).fall_distance > 0.0 {
            self.on_changed_block(id, pos);
            let fall_power = self.entity(id).fall_distance + 1.0e-6 - self.attribute_value(id, Attribute::SafeFallDistance);
            let power = mth::floor(fall_power).max(0) as f64;
            if power > 0.0 && !on_state.is_air() {
                let e = self.entity(id);
                let (mut x, y, mut z) = (e.x(), e.y(), e.z());
                let entity_pos = e.block_position();
                if pos.x != entity_pos.x || pos.z != entity_pos.z {
                    let x_diff = x - pos.x as f64 - 0.5;
                    let z_diff = z - pos.z as f64 - 0.5;
                    let max_diff = x_diff.abs().max(z_diff.abs());
                    x = pos.x as f64 + 0.5 + x_diff / max_diff * 0.5;
                    z = pos.z as f64 + 0.5 + z_diff / max_diff * 0.5;
                }
                let scale = (0.2f32 as f64 + power / 15.0).min(2.5);
                let particles = (150.0 * scale) as i32;
                self.send_block_particles(on_state, x, y, z, particles, (0.0, 0.0, 0.0), 0.15f32 as f64);
            }
        }
        let e = self.entity_mut(id);
        if !e.is_in_water() && ya < 0.0 {
            e.fall_distance -= (ya as f32) as f64;
        }
        if on_ground {
            let fall_distance = e.fall_distance;
            if fall_distance > 0.0 {
                block::of(on_state).fall_on(self, on_state, pos, id, fall_distance);
                if let Some(e) = self.entity_ref(id) {
                    let position = e.position();
                    let affected_state = e.main_supporting_block_pos.map_or(on_state, |block_pos| self.block_state(block_pos));
                    self.game_event(game_event::HIT_GROUND, position, Context::of_entity_and_state(Some(id), Some(affected_state)));
                }
            }
            if self.entity_ref(id).is_some() {
                self.reset_fall_distance(id);
            }
        }
        if living && on_ground && let Some(p) = self.players.get_mut(&id) {
            p.living.last_climbable_pos = None;
        }
    }

    pub fn do_check_fall_damage(&mut self, id: EntityId, xa: f64, ya: f64, za: f64, on_ground: bool) {
        if self.touching_unloaded_chunk(id) {
            return;
        }
        self.check_supporting_block(id, on_ground, Some(Vec3::new(xa, ya, za)));
        let pos = self.on_pos_legacy(id);
        let state = self.block_state(pos);
        self.check_fall_damage(id, ya, on_ground, state, pos);
    }

    pub fn touching_unloaded_chunk(&self, id: EntityId) -> bool {
        let b = self.entity(id).bounding_box().inflate_all(1.0);
        !self.has_chunks_at(mth::floor(b.min_x), mth::floor(b.min_z), mth::ceil(b.max_x), mth::ceil(b.max_z))
    }

    pub fn cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: crate::world::damagesource::DamageSource) -> bool {
        if let Some(LevelEntity::FallingBlock(_)) = self.entities.get(&id) {
            return self.falling_block_cause_fall_damage(id, fall_distance);
        }
        if self.is_player(id) {
            return self.player_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Creeper(_))) {
            return self.creeper_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Breeze(_))) {
            self.breeze_on_cause_fall_damage(id, fall_distance);
        }
        if self.is_llama(id) {
            return self.llama_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        }
        if matches!(self.entities.get(&id), Some(LevelEntity::Minecart(minecart)) if matches!(minecart.kind, super::vehicle::minecart::abstract_minecart::MinecartKind::Tnt(_))) {
            return self.minecart_tnt_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        }
        if self.is_abstract_horse(id) {
            return self.abstract_horse_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        }
        if self.is_living(id) {
            return self.living_entity_cause_fall_damage(id, fall_distance, damage_modifier, damage_source);
        }
        self.entity_cause_fall_damage(id, fall_distance, damage_modifier, damage_source)
    }

    pub fn entity_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: crate::world::damagesource::DamageSource) -> bool {
        if entity_type_tags::FALL_DAMAGE_IMMUNE.contains(self.entity(id).entity_type as usize) {
            return false;
        }
        self.propagate_fall_to_passengers(id, fall_distance, damage_modifier, damage_source);
        false
    }

    pub fn player_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: crate::world::damagesource::DamageSource) -> bool {
        if self.player(id).abilities_may_fly() {
            return false;
        }
        if fall_distance >= 2.0 {
            self.award_stat(id, crate::stats::stats::FALL_ONE_CM, mth::java_round(fall_distance * 100.0) as i32);
        }
        self.living_entity_cause_fall_damage(id, fall_distance, damage_modifier, damage_source)
    }

    pub fn living_entity_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: crate::world::damagesource::DamageSource) -> bool {
        let y = self.entity(id).y();
        let living = self.living_mut(id);
        let effective_fall_distance = match living.current_impulse_impact_pos {
            Some(impact) => {
                let effective_fall_distance = fall_distance.min(impact.y - y);
                if effective_fall_distance <= 0.0 {
                    living.reset_current_impulse_context();
                } else {
                    living.try_reset_current_impulse_context();
                }
                effective_fall_distance
            }
            None => fall_distance,
        };
        let damaged = self.entity_cause_fall_damage(id, effective_fall_distance, damage_modifier, damage_source);
        let dmg = self.calculate_fall_damage(id, effective_fall_distance, damage_modifier);
        if dmg > 0 {
            self.living_mut(id).reset_current_impulse_context();
            let sound = self.get_fall_damage_sound(id, dmg);
            self.entity_play_sound(id, sound_event(sound), 1.0, 1.0);
            self.play_block_fall_sound(id);
            self.hurt(id, damage_source, dmg as f32);
            return true;
        }
        damaged
    }

    pub fn propagate_fall_to_passengers(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: crate::world::damagesource::DamageSource) {
        for passenger in self.passengers(id).to_vec() {
            if self.entity_ref(passenger).is_some() {
                self.cause_fall_damage(passenger, fall_distance, damage_modifier, damage_source);
            }
        }
    }

    pub fn get_fall_damage_sound(&self, id: EntityId, dmg: i32) -> &'static str {
        let (small, big) = self.fall_sounds(id);
        if dmg > 4 { big } else { small }
    }

    pub fn fall_sounds(&self, id: EntityId) -> (&'static str, &'static str) {
        if self.is_armor_stand(id) {
            let fall = super::decoration::armor_stand::get_fall_sound();
            return (fall, fall);
        }
        if self.is_player(id) {
            ("minecraft:entity.player.small_fall", "minecraft:entity.player.big_fall")
        } else if self.is_monster(id) {
            self.monster_get_fall_sounds()
        } else {
            ("minecraft:entity.generic.small_fall", "minecraft:entity.generic.big_fall")
        }
    }

    pub fn calculate_fall_damage(&self, id: EntityId, fall_distance: f64, damage_modifier: f32) -> i32 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::Goat(_)) => self.goat_calculate_fall_damage(id, fall_distance, damage_modifier),
            Some(super::mob::MobKind::Frog(_)) => self.frog_calculate_fall_damage(id, fall_distance, damage_modifier),
            _ => self.living_entity_calculate_fall_damage(id, fall_distance, damage_modifier),
        }
    }

    pub fn living_entity_calculate_fall_damage(&self, id: EntityId, fall_distance: f64, damage_modifier: f32) -> i32 {
        if entity_type_tags::FALL_DAMAGE_IMMUNE.contains(self.entity(id).entity_type as usize) {
            return 0;
        }
        let base_damage = fall_distance + 1.0e-6 - self.attribute_value(id, Attribute::SafeFallDistance);
        mth::floor(base_damage * damage_modifier as f64 * self.attribute_value(id, Attribute::FallDamageMultiplier))
    }

    pub fn play_block_fall_sound(&mut self, id: EntityId) {
        let e = self.entity(id);
        if e.is_silent() {
            return;
        }
        let pos = BlockPos::new(mth::floor(e.x()), mth::floor(e.y() - 0.2f32 as f64), mth::floor(e.z()));
        let state = self.block_state(pos);
        if !state.is_air() {
            let sound = state.sound_type();
            self.entity_play_sound(id, sound.fall_sound, sound.volume * 0.5, sound.pitch * 0.75);
        }
    }

    fn restitute_movement_after_collisions(&mut self, id: EntityId, effect_state: BlockState, x_collision: bool, z_collision: bool, movement: Vec3) {
        let suppressing = self.entity(id).is_suppressing_bounce();
        let mut restitution = if suppressing { 0.0 } else { self.entity_bounciness(id) };
        let effective_gravity = self.effective_gravity(id);
        let air_drag = self.air_drag(id);
        let living = self.is_living(id);
        let e = self.entity(id);
        let current = e.delta_movement;
        let mut after = current;
        if x_collision {
            after = after.with(Axis::X, -current.x * restitution);
        }
        if z_collision {
            after = after.with(Axis::Z, -current.z * restitution);
        }
        let mut bounced = restitution > 0.0 && (x_collision || z_collision);
        if e.vertical_collision {
            if e.vertical_collision_below {
                restitution = if !(-current.y <= effective_gravity) && !suppressing && !effect_state.is(&block_tags::SUPPRESSES_BOUNCE) {
                    let mut block_bounciness = effect_state.block().info().bounce_restitution;
                    if !living {
                        block_bounciness *= 0.8;
                    }
                    mth::jmax(restitution, block_bounciness as f64)
                } else {
                    0.0
                };
            }
            let (gravity_compensation, effective_drag) = if restitution > 0.0 {
                let portion = movement.y / current.y;
                bounced = true;
                (portion * effective_gravity, mth::lerp(portion, 1.0, air_drag as f64))
            } else {
                (0.0, 1.0)
            };
            after = after.with(Axis::Y, (gravity_compensation - current.y) * effective_drag * restitution);
        }
        if bounced {
            let pos = self.block_pos_below_that_affects_my_movement(id);
            let fall_distance = self.entity(id).fall_distance;
            block::of(effect_state).bounce_on(self, effect_state, pos, id, fall_distance);
            if self.entity_ref(id).is_some() {
                self.entity_game_event(id, game_event::BOUNCE);
                self.entity_mut(id).sync_position = true;
            }
        }
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).set_delta_movement(after);
        }
    }

    fn is_state_climbable(state: BlockState) -> bool {
        state.is(&block_tags::CLIMBABLE) || state.is_block(blocks::POWDER_SNOW)
    }

    fn apply_movement_emission_and_play_sound(&mut self, id: EntityId, emission: MovementEmission, movement: Vec3, effect_pos: BlockPos, effect_state: BlockState) {
        let moved_distance = (movement.length() * 0.6f32 as f64) as f32;
        let horizontal_moved_distance = (movement.horizontal_distance() * 0.6f32 as f64) as f32;
        let supporting_pos = self.on_pos(id, ON_POS);
        let supporting_state = self.block_state(supporting_pos);
        let climbing = Self::is_state_climbable(supporting_state);
        let e = self.entity_mut(id);
        e.move_dist += if climbing { moved_distance } else { horizontal_moved_distance };
        e.fly_dist += moved_distance;
        if e.move_dist > e.next_step && !supporting_state.is_air() {
            let only_effect_state = supporting_pos == effect_pos;
            let mut produced = self.vibration_and_sound_effects_from_block(id, effect_pos, effect_state, emission.emits_sounds(), only_effect_state, movement);
            if !only_effect_state {
                produced |= self.vibration_and_sound_effects_from_block(id, supporting_pos, supporting_state, false, emission.emits_events(), movement);
            }
            if produced {
                let next_step = self.next_step(id);
                self.entity_mut(id).next_step = next_step;
            } else if self.entity(id).is_in_water() {
                let next_step = self.next_step(id);
                self.entity_mut(id).next_step = next_step;
                if emission.emits_sounds() {
                    self.water_swim_sound(id);
                }
                if emission.emits_events() {
                    self.entity_game_event(id, game_event::SWIM);
                }
            }
        } else if supporting_state.is_air() {
            self.process_flapping_movement(id);
        }
    }

    pub(crate) fn process_flapping_movement(&mut self, id: EntityId) {
        if self.is_flapping(id) {
            self.on_flap(id);
            if self.movement_emission(id).emits_events() {
                self.entity_game_event(id, game_event::FLAP);
            }
        }
    }

    fn is_flapping(&self, id: EntityId) -> bool {
        let Some(mob) = self.mob(id) else { return false };
        match mob.kind {
            super::mob::MobKind::Parrot(_) => self.parrot_is_flapping(id),
            super::mob::MobKind::Bee(_) => self.bee_is_flapping(id),
            super::mob::MobKind::Chicken(_) => self.chicken_is_flapping(id),
            super::mob::MobKind::Bat(_) => !mob.is_resting() && mob.entity.tick_count % 10 == 0,
            super::mob::MobKind::Allay(_) => !mob.entity.on_ground,
            super::mob::MobKind::Phantom(_) => (super::monster::phantom::get_unique_flap_tick_offset(id).wrapping_add(mob.entity.tick_count)) % super::monster::phantom::TICKS_PER_FLAP == 0,
            super::mob::MobKind::Vex(_) => mob.entity.tick_count % super::monster::vex::TICKS_PER_FLAP == 0,
            super::mob::MobKind::EnderDragon(ref dragon) => dragon.is_flapping(),
            _ => false,
        }
    }

    fn on_flap(&mut self, id: EntityId) {
        if self.is_parrot(id) {
            self.parrot_on_flap(id);
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Chicken(_))) {
            self.chicken_on_flap(id);
        }
    }

    pub fn omnidirectional_air_mover(&self, id: EntityId) -> bool {
        self.is_parrot(id) || self.is_bee(id) || self.mob(id).is_some_and(|mob| mob.sulfur_cube().is_some() && mob.has_body_item())
    }

    fn next_step(&self, id: EntityId) -> f32 {
        if self.is_turtle(id) {
            super::animal::turtle::turtle::Turtle::next_step(self.entity(id))
        } else if self.is_strider(id) {
            super::monster::strider::next_step(self.entity(id))
        } else if self.is_warden(id) {
            super::monster::warden::warden::next_step(self.entity(id))
        } else {
            self.entity_next_step(id)
        }
    }

    fn entity_next_step(&self, id: EntityId) -> f32 {
        (self.entity(id).move_dist as i32 + 1) as f32
    }

    fn vibration_and_sound_effects_from_block(&mut self, id: EntityId, pos: BlockPos, state: BlockState, should_sound: bool, should_vibrate: bool, movement: Vec3) -> bool {
        if state.is_air() {
            return false;
        }
        let climbable = Self::is_state_climbable(state);
        let on_rails = matches!(self.entities.get(&id), Some(LevelEntity::Minecart(minecart)) if minecart.is_on_rails());
        let e = self.entity(id);
        if (e.on_ground || climbable || e.is_crouching() && movement.y == 0.0 || on_rails) && !e.is_swimming() {
            if should_sound {
                self.walking_step_sound(id, pos, state);
            }
            if should_vibrate {
                let position = self.entity(id).position();
                self.game_event(game_event::STEP, position, Context::of_entity_and_state(Some(id), Some(state)));
            }
            return true;
        }
        false
    }

    fn walking_step_sound(&mut self, id: EntityId, pos: BlockPos, state: BlockState) {
        self.play_step_sound(id, pos, state);
        let e = self.entity(id);
        if state.is(&block_tags::CRYSTAL_SOUND_BLOCKS) && e.tick_count >= e.last_crystal_sound_play_tick + 20 {
            let e = self.entity_mut(id);
            e.crystal_sound_intensity *= 0.997f64.powf((e.tick_count - e.last_crystal_sound_play_tick) as f64) as f32;
            e.crystal_sound_intensity = (e.crystal_sound_intensity + 0.07).min(1.0);
            let pitch = 0.5 + e.crystal_sound_intensity * e.random.next_float() * 1.2;
            let volume = 0.1 + e.crystal_sound_intensity * 1.2;
            self.entity_play_sound(id, sound_event("minecraft:block.amethyst_block.chime"), volume, pitch);
            let e = self.entity_mut(id);
            e.last_crystal_sound_play_tick = e.tick_count;
        }
    }

    fn play_step_sound(&mut self, id: EntityId, pos: BlockPos, state: BlockState) {
        if self.is_bee(id) {
            return;
        }
        if self.mob(id).is_some_and(|mob| mob.sulfur_cube().is_some() && mob.has_body_item()) {
            return;
        }
        if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::IronGolem(_))) {
            self.iron_golem_play_step_sound(id, pos);
            return;
        }
        if self.is_copper_golem(id) {
            self.copper_golem_play_step_sound(id);
            return;
        }
        if self.is_allay(id) {
            return;
        }
        if self.mob(id).is_some_and(super::mob::Mob::is_abstract_fish) {
            return;
        }
        if self.is_camel(id) {
            self.camel_play_step_sound(id, state);
            return;
        }
        if self.is_abstract_nautilus(id) || self.is_happy_ghast(id) {
            return;
        }
        if self.is_creaking(id) {
            self.creaking_play_step_sound(id);
            return;
        }
        if self.is_warden(id) {
            self.warden_play_step_sound(id);
            return;
        }
        if self.is_abstract_horse(id) {
            self.abstract_horse_play_step_sound(id, pos, state);
            return;
        }
        if self.is_strider(id) {
            self.strider_play_step_sound(id);
            return;
        }
        if let Some(sound) = self.mob(id).and_then(|mob| mob.get_step_sound()) {
            self.entity_play_sound(id, sound_event(sound), 0.15, 1.0);
            return;
        }
        if self.is_player(id) {
            if self.entity(id).is_in_water() {
                self.water_swim_sound(id);
                let sound = state.sound_type();
                self.entity_play_sound(id, sound.step_sound, sound.volume * 0.05, sound.pitch * 0.8);
                return;
            }
            let above = pos.above();
            let above_state = self.block_state(above);
            let primary = if !above_state.is(&block_tags::INSIDE_STEP_SOUND_BLOCKS) && !above_state.is(&block_tags::COMBINATION_STEP_SOUND_BLOCKS) { pos } else { above };
            if primary != pos {
                let primary_state = self.block_state(primary);
                if primary_state.is(&block_tags::COMBINATION_STEP_SOUND_BLOCKS) {
                    let primary_sound = primary_state.sound_type();
                    self.entity_play_sound(id, primary_sound.step_sound, primary_sound.volume * 0.15, primary_sound.pitch);
                    let secondary = state.sound_type();
                    self.entity_play_sound(id, secondary.step_sound, secondary.volume * 0.05, secondary.pitch * 0.8);
                    return;
                }
                let sound = primary_state.sound_type();
                self.entity_play_sound(id, sound.step_sound, sound.volume * 0.15, sound.pitch);
                return;
            }
        }
        let sound = state.sound_type();
        self.entity_play_sound(id, sound.step_sound, sound.volume * 0.15, sound.pitch);
    }

    fn water_swim_sound(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        let speed = (((m.x * m.x * 0.2f32 as f64 + m.y * m.y + m.z * m.z * 0.2f32 as f64).sqrt() as f32) * 0.35).min(1.0);
        self.play_swim_sound(id, speed);
    }

    fn play_swim_sound(&mut self, id: EntityId, volume: f32) {
        if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::SkeletonHorse(_))) {
            let volume = self.skeleton_horse_swim_sound_volume(id, volume);
            self.entity_play_swim_sound(id, volume);
        } else if self.is_turtle(id) {
            self.entity_play_swim_sound(id, volume * 1.5);
        } else {
            self.entity_play_swim_sound(id, volume);
        }
    }

    fn entity_play_swim_sound(&mut self, id: EntityId, speed: f32) {
        let e = self.entity_mut(id);
        let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.4;
        let sound = if self.is_player(id) {
            "minecraft:entity.player.swim"
        } else if let Some(mob) = self.mob(id).filter(|mob| mob.abstract_nautilus().is_some()) {
            super::animal::nautilus::abstract_nautilus::get_swim_sound(mob)
        } else if self.is_monster(id) || self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Hoglin(_))) {
            if self.is_drowned(id) { super::monster::zombie::drowned::Drowned::get_swim_sound() } else { self.monster_get_swim_sound() }
        } else if self.mob(id).is_some_and(super::mob::Mob::is_abstract_fish) {
            super::animal::fish::abstract_fish::AbstractFish::get_swim_sound()
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Axolotl(_))) {
            super::animal::axolotl::axolotl::get_swim_sound()
        } else if self.is_dolphin(id) {
            super::animal::dolphin::dolphin::get_swim_sound()
        } else if self.is_turtle(id) {
            super::animal::turtle::turtle::Turtle::get_swim_sound()
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::SkeletonHorse(_))) {
            self.skeleton_horse_get_swim_sound(id)
        } else {
            "minecraft:entity.generic.swim"
        };
        self.entity_play_sound(id, sound_event(sound), speed, pitch);
    }

    pub fn apply_effects_from_blocks(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let mut movements = std::mem::take(&mut e.final_movements_this_tick);
        movements.clear();
        movements.extend(e.movement_this_tick.drain(..));
        match movements.last() {
            None => movements.push(Movement { from: e.old_position(), to: e.position(), axis_dependent_original_movement: None }),
            Some(last) if last.to.distance_to_sqr(e.position()) > 9.9999994e-11f32 as f64 => {
                movements.push(Movement { from: last.to, to: e.position(), axis_dependent_original_movement: None });
            }
            Some(_) => {}
        }
        self.apply_effects_from_movements(id, &movements);
        self.restore_final_movements(id, movements);
    }

    pub fn apply_effects_from_blocks_for_last_movements(&mut self, id: EntityId) {
        let movements = std::mem::take(&mut self.entity_mut(id).final_movements_this_tick);
        self.apply_effects_from_movements(id, &movements);
        self.restore_final_movements(id, movements);
    }

    fn restore_final_movements(&mut self, id: EntityId, movements: Vec<Movement>) {
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).final_movements_this_tick = movements;
        }
    }

    fn is_affected_by_blocks(&self, id: EntityId) -> bool {
        if self.is_vex(id) {
            return self.vex_is_affected_by_blocks(id);
        }
        self.entity_ref(id).is_some_and(|e| !e.is_removed() && !e.no_physics)
    }

    fn apply_effects_from_movements(&mut self, id: EntityId, movements: &[Movement]) {
        if !self.is_affected_by_blocks(id) {
            return;
        }
        if self.entity(id).on_ground {
            let effect_pos = self.on_pos_legacy(id);
            let state = self.block_state(effect_pos);
            block::of(state).step_on(self, effect_pos, state, id);
            if self.entity_ref(id).is_none() {
                return;
            }
        }
        let was_on_fire = self.is_on_fire(id);
        let was_freezing = self.entity(id).is_freezing();
        let previous_fire_ticks = self.entity(id).remaining_fire_ticks;
        self.check_inside_blocks(id, movements);
        self.apply_inside_effects(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        if self.is_in_rain(id) {
            self.clear_fire(id);
        }
        if was_on_fire && !self.is_on_fire(id) || was_freezing && !self.entity(id).is_freezing() {
            self.play_entity_on_fire_extinguished_sound(id);
        }
        let was_ignited_this_tick = self.entity(id).remaining_fire_ticks > previous_fire_ticks;
        if !self.is_on_fire(id) && !was_ignited_this_tick {
            let ticks = -self.fire_immune_ticks(id);
            self.set_remaining_fire_ticks(id, ticks);
        }
    }

    pub fn play_entity_on_fire_extinguished_sound(&mut self, id: EntityId) {
        if !self.as_projectile(id).is_some_and(|projectile| projectile.is_fireball()) {
            self.entity_play_entity_on_fire_extinguished_sound(id);
        }
    }

    pub fn entity_play_entity_on_fire_extinguished_sound(&mut self, id: EntityId) {
        let source = self.sound_source(id);
        let e = self.entity_mut(id);
        let pitch = 1.6 + (e.random.next_float() - e.random.next_float()) * 0.4;
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.generic.extinguish_fire"), source, 0.7, pitch);
    }

    pub fn apply_inside_effects(&mut self, id: EntityId) {
        if self.entity_ref(id).is_none() {
            return;
        }
        let effects = self.entity_mut(id).inside_effects.take_final();
        for effect in effects {
            if !self.is_alive(id) {
                break;
            }
            match effect {
                QueuedEffect::Type(ty) => self.apply_inside_block_effect_type(id, ty),
                QueuedEffect::Callback(f) => f(self, id),
            }
        }
    }

    fn apply_inside_block_effect_type(&mut self, id: EntityId, ty: InsideBlockEffectType) {
        match ty {
            InsideBlockEffectType::Freeze => {
                let can_freeze = self.can_freeze(id);
                let e = self.entity_mut(id);
                e.is_in_powder_snow = true;
                if can_freeze {
                    let ticks = (e.ticks_frozen() + 1).min(140);
                    e.set_ticks_frozen(ticks);
                }
            }
            InsideBlockEffectType::ClearFreeze => self.entity_mut(id).set_ticks_frozen(0),
            InsideBlockEffectType::FireIgnite => self.fire_ignite(id),
            InsideBlockEffectType::LavaIgnite => {
                if !self.fire_immune(id) {
                    self.ignite_for_ticks(id, mth::floor_f(15.0 * 20.0));
                }
            }
            InsideBlockEffectType::Extinguish => self.clear_fire(id),
        }
    }

    fn fire_ignite(&mut self, id: EntityId) {
        if self.fire_immune(id) {
            return;
        }
        let ticks = self.entity(id).remaining_fire_ticks;
        if ticks < 0 {
            self.set_remaining_fire_ticks(id, ticks + 1);
        } else if self.is_player(id) {
            let added = 1 + self.random.next_int_bounded(2);
            self.set_remaining_fire_ticks(id, ticks + added);
        }
        if self.entity(id).remaining_fire_ticks >= 0 {
            self.ignite_for_ticks(id, mth::floor_f(8.0 * 20.0));
        }
    }

    fn check_inside_blocks(&mut self, id: EntityId, movements: &[Movement]) {
        if !self.is_affected_by_blocks(id) {
            return;
        }
        let mut visited: FastSet<BlockPos> = FastSet::default();
        for movement in movements {
            let mut pos = movement.from;
            let delta = movement.to.subtract(movement.from);
            let mut max_iterations = 16;
            match movement.axis_dependent_original_movement {
                Some(original) if delta.length_sqr() > 0.0 => {
                    for axis in Axis::step_order(original.x, original.z) {
                        let axis_move = delta.get(axis);
                        if axis_move != 0.0 {
                            let to = pos.relative(axis.positive(), axis_move);
                            max_iterations -= self.check_inside_blocks_between(id, pos, to, &mut visited, max_iterations);
                            pos = to;
                        }
                    }
                }
                _ => max_iterations -= self.check_inside_blocks_between(id, movement.from, movement.to, &mut visited, 16),
            }
            if max_iterations <= 0 {
                self.check_inside_blocks_between(id, movement.to, movement.to, &mut visited, 1);
            }
        }
    }

    fn on_inside_block(&mut self, id: EntityId, state: wafer_data::BlockState) {
        if self.is_player(id) {
            crate::advancements::triggers::criteria_triggers::ENTER_BLOCK.trigger(self, id, state);
        } else if self.as_projectile(id).is_some_and(|projectile| matches!(projectile.kind, super::projectile::projectile::ProjectileKind::ThrownEnderpearl(_))) {
            self.thrown_enderpearl_on_inside_block(id, state);
        }
    }

    pub fn collided_with_shape_moving_from(&self, id: EntityId, from: Vec3, to: Vec3, aabbs: &[AABB]) -> bool {
        let bounding_box_at_from = self.entity(id).make_bounding_box(from);
        let travel_vector = to.subtract(from);
        bounding_box_at_from.collided_along_vector(travel_vector, aabbs)
    }

    pub fn collided_with_fluid(&mut self, id: EntityId, fluid_state: wafer_data::FluidState, pos: BlockPos, from: Vec3, to: Vec3) -> bool {
        match self.fluid_aabb(fluid_state, pos) {
            Some(fluid_aabb) => self.collided_with_shape_moving_from(id, from, to, &[fluid_aabb]),
            None => false,
        }
    }

    fn check_inside_blocks_between(&mut self, id: EntityId, from: Vec3, to: Vec3, visited: &mut FastSet<BlockPos>, max_iterations: i32) -> i32 {
        let e = self.entity_mut(id);
        let deflated = e.make_bounding_box(to).deflate_all(1.0e-5f32 as f64);
        let moved_far = from.distance_to_sqr(to) > mth::square(0.9999900000002526);
        let mut effects = std::mem::take(&mut e.inside_effects);
        let mut iterations = 0;
        let box_from = self.entity(id).make_bounding_box(from);
        let travel = to.subtract(from);
        for_each_block_intersected_between(from, to, &deflated, |pos, iteration| {
            if !self.is_alive(id) {
                return false;
            }
            if iteration >= max_iterations {
                return false;
            }
            iterations = iteration;
            let state = self.block_state(pos);
            if state.is_air() {
                return true;
            }
            let behaviour = block::of(state);
            let inside_block = match behaviour.entity_inside_collision_shape(state, self, pos, id) {
                None => true,
                Some(boxes) => {
                    let moved: Vec<AABB> = boxes.iter().map(|b| b.move_to_block(pos)).collect();
                    box_from.collided_along_vector(travel, &moved)
                }
            };
            let fluid = state.fluid_state();
            let inside_fluid = match self.fluid_aabb(fluid, pos) {
                Some(fluid_box) => box_from.collided_along_vector(travel, &[fluid_box]),
                None => false,
            };
            if (inside_block || inside_fluid) && visited.insert(pos) {
                if inside_block {
                    let is_precise = moved_far || deflated.intersects_block(pos);
                    effects.advance_step(iteration);
                    behaviour.entity_inside(state, self, pos, id, &mut effects, is_precise);
                    self.on_inside_block(id, state);
                }
                if inside_fluid {
                    effects.advance_step(iteration);
                    crate::world::level::material::entity_inside(fluid, &mut effects);
                }
            }
            true
        });
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).inside_effects = effects;
        }
        iterations + 1
    }

    pub fn update_fluid_interaction(&mut self, id: EntityId) -> bool {
        let pushed = self.is_pushed_by_fluid(id);
        let in_any_fluid = self.update_fluid_trackers(id, !pushed);
        let e = self.entity(id);
        let in_water = e.fluid_interaction.is_in_fluid(&fluid_tags::WATER);
        let in_lava = e.fluid_interaction.is_in_fluid(&fluid_tags::LAVA);
        if in_water {
            self.reset_fall_distance(id);
            let e = self.entity(id);
            if !e.was_touching_water && !e.first_tick {
                self.do_water_splash_effect(id);
            }
        }
        self.entity_mut(id).was_touching_water = in_water;
        if pushed {
            if in_water {
                self.apply_fluid_current(id, CurrentFluid::Water, 0.014);
            }
            if in_lava {
                let fast_lava = self.environment_attributes().get_dimension_value(environment_attributes::FAST_LAVA);
                self.apply_fluid_current(id, CurrentFluid::Lava, if fast_lava { 0.007 } else { 0.0023333333333333335 });
            }
        }
        in_any_fluid
    }

    pub fn do_water_splash_effect(&mut self, id: EntityId) {
        if self.is_spectator(id) || matches!(self.entities.get(&id), Some(super::LevelEntity::ExperienceOrb(_))) {
            return;
        }
        let player = self.is_player(id);
        let monster = self.is_monster(id);
        let axolotl = self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Axolotl(_)));
        let dolphin = self.is_dolphin(id);
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        let speed = (((m.x * m.x * 0.2f32 as f64 + m.y * m.y + m.z * m.z * 0.2f32 as f64).sqrt() as f32) * 0.2).min(1.0);
        let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.4;
        let sound = match (player, speed < 0.25) {
            (true, true) => "minecraft:entity.player.splash",
            (true, false) => "minecraft:entity.player.splash.high_speed",
            (false, true) if monster => self.monster_get_swim_splash_sound(),
            (false, true) if axolotl => super::animal::axolotl::axolotl::get_swim_splash_sound(),
            (false, true) if dolphin => super::animal::dolphin::dolphin::get_swim_splash_sound(),
            (false, _) => "minecraft:entity.generic.splash",
        };
        self.entity_play_sound(id, sound_event(sound), speed, pitch);
        let e = self.entity_mut(id);
        let count = 1.0 + e.dimensions.width * 20.0;
        let mut i = 0;
        while (i as f32) < count {
            e.random.next_double();
            e.random.next_double();
            e.random.next_double();
            i += 1;
        }
        let mut i = 0;
        while (i as f32) < count {
            e.random.next_double();
            e.random.next_double();
            i += 1;
        }
        self.entity_game_event(id, game_event::SPLASH);
    }

    pub fn update_swimming(&mut self, id: EntityId) {
        if self.is_drowned(id) {
            self.drowned_update_swimming(id);
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.abilities_flying()) {
            self.entity_mut(id).set_swimming(false);
            return;
        }
        let e = self.entity(id);
        let swimming = if e.is_swimming() {
            e.is_sprinting() && e.is_in_water()
        } else {
            e.is_sprinting() && e.is_under_water() && {
                let pos = e.block_position();
                self.fluid_state(pos).is(&fluid_tags::WATER)
            }
        };
        self.entity_mut(id).set_swimming(swimming);
    }

    fn can_spawn_sprint_particle(&mut self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::IronGolem(_)) => self.iron_golem_can_spawn_sprint_particle(id),
            Some(super::mob::MobKind::Rabbit(_)) => super::animal::rabbit::rabbit::Rabbit::can_spawn_sprint_particle(),
            _ => self.entity_can_spawn_sprint_particle(id),
        }
    }

    fn entity_can_spawn_sprint_particle(&self, id: EntityId) -> bool {
        let e = self.entity(id);
        e.is_sprinting() && !e.is_in_water() && !self.is_spectator(id) && !e.is_crouching() && !e.is_in_lava() && self.is_alive(id)
    }

    fn spawn_sprint_particle(&mut self, id: EntityId) {
        let pos = self.on_pos_legacy(id);
        let block_state = self.block_state(pos);
        if block_behaviour::get_render_shape(block_state) != RenderShape::Invisible {
            let random = &mut self.entity_mut(id).random;
            random.next_double();
            random.next_double();
        }
    }

    pub fn entity_base_tick(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.compute_speed();
        e.in_block_state = None;
        if let Some(vehicle) = self.vehicle(id)
            && self.entity_ref(vehicle).is_none_or(Entity::is_removed)
        {
            self.stop_riding(id);
        }
        let e = self.entity_mut(id);
        if e.boarding_cooldown > 0 {
            e.boarding_cooldown -= 1;
        }
        self.handle_portal(id);
        if self.can_spawn_sprint_particle(id) {
            self.spawn_sprint_particle(id);
        }
        let e = self.entity_mut(id);
        e.was_in_powder_snow = e.is_in_powder_snow;
        e.is_in_powder_snow = false;
        e.was_eye_in_water = e.is_eye_in_fluid(&fluid_tags::WATER);
        self.update_fluid_interaction(id);
        self.update_swimming(id);
        let remaining_fire_ticks = self.entity(id).remaining_fire_ticks;
        if remaining_fire_ticks > 0 {
            if self.fire_immune(id) {
                self.clear_fire(id);
            } else {
                if remaining_fire_ticks % 20 == 0 && !self.entity(id).is_in_lava() {
                    self.hurt_server(id, crate::world::damagesource::DamageSource::new(wafer_data::damage_types::ON_FIRE), 1.0);
                }
                let ticks = self.entity(id).remaining_fire_ticks - 1;
                self.set_remaining_fire_ticks(id, ticks);
            }
        }
        let e = self.entity_mut(id);
        if e.is_in_lava() {
            e.fall_distance *= 0.5;
        }
        self.check_below_world(id);
        let e = self.entity_mut(id);
        let on_fire = e.remaining_fire_ticks > 0;
        e.set_shared_flag(flags::ON_FIRE, on_fire);
        e.first_tick = false;
        if self.is_leashable(id) {
            self.tick_leash(id);
        }
    }

    pub fn on_above_bubble_column(&mut self, id: EntityId, drag_down: bool, pos: BlockPos) {
        if self.is_boat(id) {
            self.abstract_boat_on_above_bubble_column(id, drag_down, pos);
            return;
        }
        if let Some(projectile) = self.as_projectile(id).filter(|_| !self.is_thrown_enderpearl(id)) {
            if projectile.abstract_arrow().is_some() && projectile.is_in_ground() {
                return;
            }
            let yd = if drag_down { -0.03 } else { 0.1 };
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, yd, 0.0));
            self.send_bubble_column_particles(pos);
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.abilities_flying()) {
            return;
        }
        if self.can_simulate_movement(id) {
            let e = self.entity_mut(id);
            let movement = e.delta_movement;
            let yd = if drag_down { (-0.9f64).max(movement.y - 0.03) } else { 1.8f64.min(movement.y + 0.1) };
            e.set_delta_movement(Vec3::new(movement.x, yd, movement.z));
        }
        self.send_bubble_column_particles(pos);
    }

    fn send_bubble_column_particles(&mut self, pos: BlockPos) {
        for _ in 0..2 {
            let (x, z) = (pos.x as f64 + self.random.next_double(), pos.z as f64 + self.random.next_double());
            self.send_particles("minecraft:splash", |_| {}, x, (pos.y + 1) as f64, z, 1, (0.0, 0.0, 0.0), 1.0);
            let (x, z) = (pos.x as f64 + self.random.next_double(), pos.z as f64 + self.random.next_double());
            self.send_particles("minecraft:bubble", |_| {}, x, (pos.y + 1) as f64, z, 1, (0.0, 0.01, 0.0), 0.2);
        }
    }

    pub fn on_inside_bubble_column(&mut self, id: EntityId, drag_down: bool) {
        if let Some(projectile) = self.as_projectile(id).filter(|_| !self.is_thrown_enderpearl(id)) {
            if projectile.abstract_arrow().is_some() && projectile.is_in_ground() {
                return;
            }
            let yd = if drag_down { -0.03 } else { 0.06 };
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, yd, 0.0));
            self.reset_fall_distance(id);
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.abilities_flying()) {
            return;
        }
        if self.can_simulate_movement(id) {
            let e = self.entity_mut(id);
            let movement = e.delta_movement;
            let yd = if drag_down { (-0.3f64).max(movement.y - 0.03) } else { 0.7f64.min(movement.y + 0.06) };
            e.set_delta_movement(Vec3::new(movement.x, yd, movement.z));
        }
        self.reset_fall_distance(id);
    }

    pub fn check_below_world(&mut self, id: EntityId) {
        if self.entity(id).y() < (self.chunks.min_y() - 64) as f64 {
            self.on_below_world(id);
        }
    }

    pub fn discard(&mut self, id: EntityId) {
        self.remove(id, RemovalReason::Discarded);
    }

    fn on_below_world(&mut self, id: EntityId) {
        if self.is_living(id) {
            self.hurt(id, crate::world::damagesource::DamageSource::new(wafer_data::damage_types::OUT_OF_WORLD), 4.0);
        } else {
            self.discard(id);
        }
    }

    pub fn lava_ignite(&mut self, id: EntityId) {
        if !self.fire_immune(id) {
            self.ignite_for_seconds(id, 15.0);
        }
    }

    pub fn apply_effects_from_blocks_between(&mut self, id: EntityId, from: Vec3, to: Vec3) {
        self.apply_effects_from_movements(id, &[Movement { from, to, axis_dependent_original_movement: None }]);
    }

    pub fn move_towards_closest_space(&mut self, id: EntityId, x: f64, y: f64, z: f64) {
        let pos = BlockPos::containing(x, y, z);
        let delta = Vec3::new(x - pos.x as f64, y - pos.y as f64, z - pos.z as f64);
        let mut closest_direction = Direction::Up;
        let mut closest = f64::MAX;
        for direction in [Direction::North, Direction::South, Direction::West, Direction::East, Direction::Up] {
            let neighbour = pos.relative(direction);
            if !self.block_state(neighbour).is_collision_shape_full_block() {
                let d = delta.get(direction.axis());
                let oriented = if direction.axis_direction() == AxisDirection::Positive { 1.0 - d } else { d };
                if oriented < closest {
                    closest = oriented;
                    closest_direction = direction;
                }
            }
        }
        let e = self.entity_mut(id);
        let speed = e.random.next_float() * 0.2 + 0.1;
        let step = closest_direction.axis_direction().step() as f32;
        let scaled = e.delta_movement.scale(0.75);
        let push = (step * speed) as f64;
        match closest_direction.axis() {
            Axis::X => e.set_delta_movement(Vec3::new(push, scaled.y, scaled.z)),
            Axis::Y => e.set_delta_movement(Vec3::new(scaled.x, push, scaled.z)),
            Axis::Z => e.set_delta_movement(Vec3::new(scaled.x, scaled.y, push)),
        }
    }

    pub fn is_free(&mut self, id: EntityId, xa: f64, ya: f64, za: f64) -> bool {
        let box_ = self.entity(id).bounding_box().move_by(xa, ya, za);
        self.no_collision_for(id, &box_) && !self.contains_any_liquid(&box_)
    }

    pub fn place_portal_ticket(&mut self, ticket_position: BlockPos) {
        self.chunks.add_ticket_with_radius(crate::server::level::ticket::TicketType::Portal, ticket_position.chunk(), 3);
    }

    pub fn teleport_relative(&mut self, id: EntityId, dx: f64, dy: f64, dz: f64) {
        if let Some(p) = self.players.get_mut(&id) {
            let destination = super::relative::PositionMoveRotation { position: Vec3::new(dx, dy, dz), delta_movement: Vec3::ZERO, y_rot: 0.0, x_rot: 0.0 };
            p.teleport_relative(destination, super::relative::relative::ALL);
        } else {
            let e = self.entity_mut(id);
            let (x, y, z, y_rot, x_rot) = (e.x() + dx, e.y() + dy, e.z() + dz, e.y_rot, e.x_rot);
            e.snap_to(x, y, z, y_rot, x_rot);
        }
    }

    pub fn make_stuck_in_block(&mut self, id: EntityId, state: BlockState, speed_multiplier: Vec3) {
        if self.is_wither(id) {
            return;
        }
        if self.mob(id).is_some_and(super::mob::Mob::is_spider) {
            self.spider_make_stuck_in_block(id, state, speed_multiplier);
        } else {
            self.entity_make_stuck_in_block(id, state, speed_multiplier);
        }
    }

    pub fn entity_make_stuck_in_block(&mut self, id: EntityId, _state: BlockState, speed_multiplier: Vec3) {
        if !self.players.get(&id).is_some_and(|p| p.abilities_flying()) {
            self.reset_fall_distance(id);
            self.entity_mut(id).stuck_speed_multiplier = speed_multiplier;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.living.try_reset_current_impulse_context();
        }
    }

    pub fn friction_below(&mut self, id: EntityId) -> f32 {
        let pos = self.block_pos_below_that_affects_my_movement(id);
        self.block_state(pos).block().info().friction
    }

    pub fn entity_tick(&mut self, id: EntityId) {
        self.base_tick(id);
    }

    pub fn base_tick(&mut self, id: EntityId) {
        if self.mob(id).is_some_and(super::mob::Mob::is_squid) {
            self.ageable_water_creature_base_tick(id);
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Axolotl(_))) {
            self.axolotl_base_tick(id);
        } else if self.is_nautilus(id) {
            self.nautilus_base_tick(id);
        } else if self.mob(id).is_some_and(super::mob::Mob::is_water_animal) {
            self.water_animal_base_tick(id);
        } else if self.is_mob(id) {
            self.mob_base_tick(id);
        } else if self.is_living(id) {
            self.living_entity_base_tick(id);
        } else {
            self.entity_base_tick(id);
        }
    }

    pub fn mark_hurt(&mut self, id: EntityId) {
        self.entity_mut(id).sync_velocity = true;
    }

    pub fn hurt(&mut self, id: EntityId, source: DamageSource, damage: f32) {
        self.hurt_server(id, source, damage);
    }

    pub fn hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_armor_stand(id) {
            return self.armor_stand_hurt_server(id, source, damage);
        }
        if self.is_player(id) {
            return self.server_player_hurt_server(id, source, damage);
        }
        if self.is_mannequin(id) {
            return self.living_entity_hurt_server(id, source, damage);
        }
        if self.is_mob(id) {
            if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Zombie(_) | super::mob::MobKind::ZombieVillager(_) | super::mob::MobKind::Husk(_) | super::mob::MobKind::Drowned(_) | super::mob::MobKind::ZombifiedPiglin(_))) {
                return self.zombie_hurt_server(id, source, damage);
            }
            if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Silverfish(_))) {
                return self.silverfish_hurt_server(id, source, damage);
            }
            if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Enderman(_))) {
                return self.enderman_hurt_server(id, source, damage);
            }
            if self.is_wolf(id) {
                return self.wolf_hurt_server(id, source, damage);
            }
            if self.mob(id).is_some_and(|mob| matches!(mob.kind, super::mob::MobKind::Armadillo(_))) {
                return self.armadillo_hurt_server(id, source, damage);
            }
            if self.mob(id).and_then(super::mob::Mob::raider).is_some() {
                return self.raider_hurt_server(id, source, damage);
            }
            match self.mob(id).map(|mob| &mob.kind) {
                Some(super::mob::MobKind::Bat(_)) => return self.bat_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Ghast(_)) => return self.ghast_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Creaking(_)) => return self.creaking_hurt_server(id, source, damage),
                Some(super::mob::MobKind::SulfurCube(_)) => return self.sulfur_cube_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Warden(_)) => return self.warden_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Guardian(_) | super::mob::MobKind::ElderGuardian(_)) => return self.guardian_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Squid(_)) => return self.squid_hurt_server(id, source, damage),
                Some(super::mob::MobKind::GlowSquid(_)) => return self.glow_squid_hurt_server(id, source, damage),
                Some(super::mob::MobKind::IronGolem(_)) => return self.iron_golem_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Allay(_)) => return self.allay_hurt_server(id, source, damage),
                Some(
                    super::mob::MobKind::Horse(_)
                    | super::mob::MobKind::Donkey(_)
                    | super::mob::MobKind::Mule(_)
                    | super::mob::MobKind::SkeletonHorse(_)
                    | super::mob::MobKind::ZombieHorse(_)
                    | super::mob::MobKind::Llama(_)
                    | super::mob::MobKind::TraderLlama(_)
                    | super::mob::MobKind::Camel(_)
                    | super::mob::MobKind::CamelHusk(_),
                ) => {
                    return self.abstract_horse_hurt_server(id, source, damage);
                }
                Some(super::mob::MobKind::Nautilus(_) | super::mob::MobKind::ZombieNautilus(_)) => return self.abstract_nautilus_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Axolotl(_)) => return self.axolotl_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Hoglin(_)) => return self.hoglin_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Panda(_)) => return self.panda_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Parrot(_)) => return self.parrot_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Bee(_)) => return self.bee_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Shulker(_)) => return self.shulker_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Wither(_)) => return self.wither_hurt_server(id, source, damage),
                Some(super::mob::MobKind::EnderDragon(_)) => return self.ender_dragon_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Zoglin(_)) => return self.zoglin_hurt_server(id, source, damage),
                Some(super::mob::MobKind::Piglin(_)) => return self.piglin_hurt_server(id, source, damage),
                Some(super::mob::MobKind::PiglinBrute(_)) => return self.piglin_brute_hurt_server(id, source, damage),
                _ => {}
            }
            return self.living_entity_hurt_server(id, source, damage);
        }
        match self.entities.get(&id) {
            Some(LevelEntity::ExperienceOrb(_)) => self.experience_orb_hurt_server(id, &source, damage),
            Some(LevelEntity::Item(_)) => self.item_entity_hurt_server(id, source, damage),
            Some(LevelEntity::FallingBlock(_)) => self.falling_block_hurt_server(id, source),
            Some(LevelEntity::EndCrystal(_)) => self.end_crystal_hurt_server(id, source),
            Some(LevelEntity::Projectile(_)) => self.projectile_hurt_server(id, &source),
            Some(LevelEntity::LeashKnot(_) | LevelEntity::Painting(_)) => self.block_attached_entity_hurt_server(id, source),
            Some(LevelEntity::ItemFrame(_)) => self.item_frame_hurt_server(id, source),
            Some(LevelEntity::Cushion(_)) => self.cushion_hurt_server(id, source),
            Some(LevelEntity::Marker(_) | LevelEntity::Interaction(_)) => super::marker::Marker::hurt_server(),
            Some(LevelEntity::OminousItemSpawner(_)) => super::ominous_item_spawner::OminousItemSpawner::hurt_server(),
            Some(LevelEntity::Display(_)) => super::display::Display::hurt_server(),
            Some(LevelEntity::Minecart(minecart)) if matches!(minecart.kind, super::vehicle::minecart::abstract_minecart::MinecartKind::Tnt(_)) => self.minecart_tnt_hurt_server(id, source, damage),
            Some(LevelEntity::Minecart(_) | LevelEntity::Boat(_)) => self.vehicle_entity_hurt_server(id, source, damage),
            None if self.dragon_parts.contains_key(id) => self.ender_dragon_part_hurt_server(id, source, damage),
            _ => false,
        }
    }

    pub fn projectile_receives_side_effects_on_hit(&self, id: EntityId, was_hurt: bool) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::Enderman(_)) => self.enderman_projectile_receives_side_effects_on_hit(was_hurt),
            _ => true,
        }
    }

    pub fn is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        if self.is_player(id) {
            self.server_player_is_invulnerable_to(id, source)
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Ghast(_))) {
            self.ghast_is_invulnerable_to(id, source)
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Breeze(_))) {
            self.breeze_is_invulnerable_to(id, source)
        } else if self.is_warden(id) {
            self.warden_is_invulnerable_to(id, source) || self.living_entity_is_invulnerable_to(id, source)
        } else if self.is_living(id) {
            self.living_entity_is_invulnerable_to(id, source)
        } else {
            self.is_invulnerable_to_base(id, source)
        }
    }

    pub fn is_invulnerable_to_base(&self, id: EntityId, source: &DamageSource) -> bool {
        let e = self.entity(id);
        let creative_player = source.causing_entity.and_then(|c| self.players.get(&c)).is_some_and(|p| p.abilities.instabuild);
        e.is_removed()
            || e.is_invulnerable() && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) && !creative_player
            || source.is(&damage_type_tags::IS_FIRE) && self.fire_immune(id)
            || source.is(&damage_type_tags::IS_FALL) && entity_type_tags::FALL_DAMAGE_IMMUNE.contains(e.entity_type as usize)
    }

    fn should_play_lava_hurt_sound(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Item(_)) => self.item_entity_should_play_lava_hurt_sound(id),
            _ => true,
        }
    }

    pub fn interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        match self.entities.get(&id) {
            Some(LevelEntity::ArmorStand(_)) => return self.armor_stand_interact(id, player, hand, location),
            Some(LevelEntity::LeashKnot(_)) => return self.leash_fence_knot_interact(id, player, hand, location),
            Some(LevelEntity::ItemFrame(_)) => return self.item_frame_interact(id, player, hand),
            Some(LevelEntity::Cushion(_)) => return self.cushion_interact(id, player, hand, location),
            Some(LevelEntity::Interaction(_)) => return self.interaction_interact(id, player),
            Some(LevelEntity::Minecart(_)) => return self.abstract_minecart_interact(id, player, hand, location),
            Some(LevelEntity::Boat(boat)) if boat.container.is_some() => return self.abstract_chest_boat_interact(id, player, hand, location),
            Some(LevelEntity::Boat(_)) => return self.abstract_boat_interact(id, player, hand, location),
            _ => {}
        }
        if !self.is_mob(id) {
            return self.entity_interact(id, player, hand, location);
        }
        if self.is_zombie_horse(id) || self.is_camel_husk(id) || self.is_abstract_nautilus(id) {
            self.mob_mut(id).expect("a mob").persistence_required = true;
        }
        if !self.is_alive(id) {
            return InteractionResult::Pass;
        }
        let interaction_result = self.check_and_handle_important_interactions(id, player, hand);
        if interaction_result.consumes_action() {
            self.entity_game_event_by(id, game_event::ENTITY_INTERACT, Some(player));
            return interaction_result;
        }
        let super_reaction = self.entity_interact(id, player, hand, location);
        if super_reaction != InteractionResult::Pass {
            return super_reaction;
        }
        let interaction_result = self.mob_interact(id, player, hand);
        if interaction_result.consumes_action() {
            self.entity_game_event_by(id, game_event::ENTITY_INTERACT, Some(player));
            interaction_result
        } else {
            InteractionResult::Pass
        }
    }

    pub fn entity_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, _location: Vec3) -> InteractionResult {
        if self.player(player).is_secondary_use_active() && self.is_leashable(id) && self.can_be_leashed(id) && self.is_alive(id) && !(self.is_living(id) && self.is_baby(id)) {
            let mobs_to_leash = {
                let center = self.entity(id).bounding_box().center();
                self.leashable_in_area(center, |level, leashable| level.get_leash_holder(leashable) == Some(player))
            };
            if !mobs_to_leash.is_empty() {
                let mut any_leashed = false;
                for mob in mobs_to_leash {
                    if self.can_have_a_leash_attached_to(mob, id) {
                        self.set_leashed_to(mob, id, true);
                        any_leashed = true;
                    }
                }
                if any_leashed {
                    let pos = self.entity(id).block_position();
                    self.game_event_at_with(game_event::ENTITY_ACTION, pos, Context::of(Some(player)));
                    self.entity_play_sound(id, sound_event("minecraft:item.lead.tied"), 1.0, 1.0);
                    return InteractionResult::SUCCESS_SERVER.without_item();
                }
            }
        }
        let held_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if held_item.is(wafer_data::items::SHEARS) && self.shear_off_all_leash_connections(id, Some(player)) {
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            return InteractionResult::SUCCESS;
        }
        if self.is_mob(id)
            && held_item.is(wafer_data::items::SHEARS)
            && self.can_shear_equipment(id, player)
            && !self.player(player).is_secondary_use_active()
            && self.attempt_to_shear_equipment(id, player, hand)
        {
            return InteractionResult::SUCCESS;
        }
        if self.is_alive(id) && self.is_leashable(id) {
            if self.get_leash_holder(id) == Some(player) {
                if self.has_infinite_materials(player) {
                    self.remove_leash(id);
                } else {
                    self.drop_leash(id);
                }
                self.entity_game_event_by(id, game_event::ENTITY_INTERACT, Some(player));
                self.entity_play_sound(id, sound_event("minecraft:item.lead.untied"), 1.0, 1.0);
                return InteractionResult::SUCCESS.without_item();
            }
            let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if item_stack.is(wafer_data::items::LEAD) && !self.get_leash_holder(id).is_some_and(|holder| self.is_player(holder)) && self.can_have_a_leash_attached_to(id, player) {
                if self.is_leashed(id) {
                    self.drop_leash(id);
                }
                self.set_leashed_to(id, player, true);
                self.entity_play_sound(id, sound_event("minecraft:item.lead.tied"), 1.0, 1.0);
                item_stack.shrink(1);
                self.player_mut(player).set_item_in_hand(hand, Some(item_stack));
                return InteractionResult::SUCCESS_SERVER;
            }
        }
        InteractionResult::Pass
    }

    pub fn blocks_building(&self, id: EntityId) -> bool {
        if let Some(stand) = self.armor_stand(id) {
            return !stand.is_marker();
        }
        self.is_living(id) || matches!(self.entities.get(&id), Some(LevelEntity::PrimedTnt(_) | LevelEntity::FallingBlock(_) | LevelEntity::Minecart(_) | LevelEntity::Boat(_)))
    }

    pub fn is_attackable(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Item(_) | LevelEntity::ExperienceOrb(_) | LevelEntity::FallingBlock(_) | LevelEntity::EyeOfEnder(_)) => false,
            Some(LevelEntity::Projectile(projectile)) => match projectile.kind {
                super::projectile::projectile::ProjectileKind::FireworkRocket(_) => false,
                super::projectile::projectile::ProjectileKind::Arrow(_)
                | super::projectile::projectile::ProjectileKind::SpectralArrow(_)
                | super::projectile::projectile::ProjectileKind::ThrownTrident(_) => {
                    entity_type_tags::REDIRECTABLE_PROJECTILE.contains(projectile.entity.entity_type as usize)
                }
                _ => true,
            },
            _ => true,
        }
    }

    pub fn is_allied_to(&self, id: EntityId, other: EntityId) -> bool {
        id == other || self.considers_entity_as_ally(id, other) || self.considers_entity_as_ally(other, id)
    }

    pub fn considers_entity_as_ally(&self, id: EntityId, other: EntityId) -> bool {
        if self.is_tamable_animal(id) {
            self.tamable_animal_considers_entity_as_ally(id, other)
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Evoker(_))) {
            self.evoker_considers_entity_as_ally(id, other)
        } else if self.is_abstract_illager(id) {
            self.abstract_illager_considers_entity_as_ally(id, other)
        } else if self.is_allay(id) {
            self.allay_considers_entity_as_ally(id, other)
        } else {
            self.entity_considers_entity_as_ally(id, other)
        }
    }

    pub fn dampens_vibrations(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Item(item)) => item.item.get_item().is(&wafer_data::item_tags::DAMPENS_VIBRATIONS),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Warden(_)) => true,
            _ => false,
        }
    }

    pub fn is_stepping_carefully(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::Cat(_)) => self.cat_is_stepping_carefully(id),
            Some(super::mob::MobKind::Ocelot(_)) => self.ocelot_is_stepping_carefully(id),
            _ => self.entity_is_stepping_carefully(id),
        }
    }

    pub fn entity_is_stepping_carefully(&self, id: EntityId) -> bool {
        self.entity(id).is_shift_key_down()
    }

    pub fn entity_considers_entity_as_ally(&self, id: EntityId, other: EntityId) -> bool {
        self.is_allied_to_team(id, self.team_of(other).as_deref())
    }

    pub fn is_allied_to_team(&self, id: EntityId, other: Option<&crate::world::scores::player_team::PlayerTeam>) -> bool {
        self.team_of(id).is_some_and(|team| team.is_allied_to(other))
    }

    pub fn kill(&mut self, id: EntityId) {
        if self.is_block_attached_entity(id) {
            self.block_attached_entity_kill(id, Some(id));
            return;
        }
        if self.is_ender_dragon(id) {
            self.ender_dragon_kill(id);
            return;
        }
        if self.is_living(id) {
            self.hurt_server(id, DamageSource::new(damage_types::GENERIC_KILL), f32::MAX);
            return;
        }
        if matches!(self.entities.get(&id), Some(LevelEntity::EndCrystal(_))) {
            self.end_crystal_kill(id);
            return;
        }
        self.entity_kill(id);
    }

    pub fn entity_kill(&mut self, id: EntityId) {
        self.remove(id, RemovalReason::Killed);
        self.entity_game_event(id, game_event::ENTITY_DIE);
    }

    pub fn set_remaining_fire_ticks(&mut self, id: EntityId, ticks: i32) {
        if self.is_player(id) {
            self.player_set_remaining_fire_ticks(id, ticks);
        } else {
            self.entity_mut(id).remaining_fire_ticks = ticks;
        }
    }

    fn is_in_rain(&mut self, id: EntityId) -> bool {
        let e = self.entity(id);
        let pos = e.block_position();
        let max_y = e.bounding_box().max_y;
        self.is_raining_at(pos) || self.is_raining_at(BlockPos::containing(pos.x as f64, max_y, pos.z as f64))
    }

    pub fn is_in_water_or_rain(&mut self, id: EntityId) -> bool {
        self.entity(id).is_in_water() || self.is_in_rain(id)
    }

    pub fn clear_fire(&mut self, id: EntityId) {
        let ticks = self.entity(id).remaining_fire_ticks.min(0);
        self.set_remaining_fire_ticks(id, ticks);
    }

    pub fn lava_hurt(&mut self, id: EntityId) {
        if self.fire_immune(id) {
            return;
        }
        if self.hurt_server(id, DamageSource::new(damage_types::LAVA), 4.0) && self.should_play_lava_hurt_sound(id) && !self.entity(id).is_silent() {
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let pitch = 2.0 + e.random.next_float() * 0.4;
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.generic.burn"), source, 0.4, pitch);
        }
    }

    pub fn is_in_wall(&mut self, id: EntityId) -> bool {
        if self.is_sleeping(id) {
            return false;
        }
        let e = self.entity(id);
        if e.no_physics {
            return false;
        }
        let check_width = (e.dimensions.width * 0.8) as f64;
        let eye = e.eye_position();
        let eye_bb = AABB::new(
            eye.x - check_width / 2.0,
            eye.y - 1.0e-6 / 2.0,
            eye.z - check_width / 2.0,
            eye.x + check_width / 2.0,
            eye.y + 1.0e-6 / 2.0,
            eye.z + check_width / 2.0,
        );
        let eye_shape = VoxelShape::from_boxes(&[eye_bb]);
        let (min, max) = (BlockPos::containing(eye_bb.min_x, eye_bb.min_y, eye_bb.min_z), BlockPos::containing(eye_bb.max_x, eye_bb.max_y, eye_bb.max_z));
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = BlockPos::new(x, y, z);
                    let state = self.block_state(pos);
                    if !state.is_air() && state.is_suffocating() {
                        let shape = state.collision_voxel_shape();
                        if join_is_not_empty(&shape.moved(pos.x as f64, pos.y as f64, pos.z as f64), &eye_shape, BooleanOp::And) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn entity_to_string(&self, id: EntityId) -> String {
        let e = self.entity(id);
        let (class, name) = if let Some(player) = self.players.get(&id) {
            ("ServerPlayer", player.profile.name.clone())
        } else {
            let name = self.entities.get(&id).map(|e| e.name().get_string()).unwrap_or_default();
            use super::projectile::projectile::ProjectileKind;
            let class = match self.entities.get(&id) {
                Some(LevelEntity::Mob(mob)) => match mob.kind {
                    super::mob::MobKind::Chicken(_) => "Chicken",
                    super::mob::MobKind::Cow(_) => "Cow",
                    super::mob::MobKind::MushroomCow(_) => "MushroomCow",
                    super::mob::MobKind::Pig(_) => "Pig",
                    super::mob::MobKind::Sheep(_) => "Sheep",
                    super::mob::MobKind::Goat(_) => "Goat",
                    super::mob::MobKind::PolarBear(_) => "PolarBear",
                    super::mob::MobKind::Armadillo(_) => "Armadillo",
                    super::mob::MobKind::Sniffer(_) => "Sniffer",
                    super::mob::MobKind::Zombie(_) => "Zombie",
                    super::mob::MobKind::Husk(_) => "Husk",
                    super::mob::MobKind::ZombieVillager(_) => "ZombieVillager",
                    super::mob::MobKind::Creeper(_) => "Creeper",
                    super::mob::MobKind::Spider(_) => "Spider",
                    super::mob::MobKind::CaveSpider(_) => "CaveSpider",
                    super::mob::MobKind::Skeleton(_) => "Skeleton",
                    super::mob::MobKind::Stray(_) => "Stray",
                    super::mob::MobKind::Parched(_) => "Parched",
                    super::mob::MobKind::Hoglin(_) => "Hoglin",
                    super::mob::MobKind::Panda(_) => "Panda",
                    super::mob::MobKind::Strider(_) => "Strider",
                    super::mob::MobKind::Dolphin(_) => "Dolphin",
                    super::mob::MobKind::Zoglin(_) => "Zoglin",
                    super::mob::MobKind::Piglin(_) => "Piglin",
                    super::mob::MobKind::PiglinBrute(_) => "PiglinBrute",
                    super::mob::MobKind::Breeze(_) => "Breeze",
                    super::mob::MobKind::Creaking(_) => "Creaking",
                    super::mob::MobKind::Warden(_) => "Warden",
                    super::mob::MobKind::WitherSkeleton(_) => "WitherSkeleton",
                    super::mob::MobKind::Bogged(_) => "Bogged",
                    super::mob::MobKind::Drowned(_) => "Drowned",
                    super::mob::MobKind::Silverfish(_) => "Silverfish",
                    super::mob::MobKind::Endermite(_) => "Endermite",
                    super::mob::MobKind::Giant(_) => "Giant",
                    super::mob::MobKind::Enderman(_) => "Enderman",
                    super::mob::MobKind::Blaze(_) => "Blaze",
                    super::mob::MobKind::Ghast(_) => "Ghast",
                    super::mob::MobKind::ZombifiedPiglin(_) => "ZombifiedPiglin",
                    super::mob::MobKind::Witch(_) => "Witch",
                    super::mob::MobKind::Ravager(_) => "Ravager",
                    super::mob::MobKind::Vindicator(_) => "Vindicator",
                    super::mob::MobKind::Pillager(_) => "Pillager",
                    super::mob::MobKind::Evoker(_) => "Evoker",
                    super::mob::MobKind::Illusioner(_) => "Illusioner",
                    super::mob::MobKind::Vex(_) => "Vex",
                    super::mob::MobKind::Phantom(_) => "Phantom",
                    super::mob::MobKind::Guardian(_) => "Guardian",
                    super::mob::MobKind::ElderGuardian(_) => "ElderGuardian",
                    super::mob::MobKind::Slime(_) => "Slime",
                    super::mob::MobKind::SulfurCube(_) => "SulfurCube",
                    super::mob::MobKind::MagmaCube(_) => "MagmaCube",
                    super::mob::MobKind::Wolf(_) => "Wolf",
                    super::mob::MobKind::Bat(_) => "Bat",
                    super::mob::MobKind::Squid(_) => "Squid",
                    super::mob::MobKind::GlowSquid(_) => "GlowSquid",
                    super::mob::MobKind::IronGolem(_) => "IronGolem",
                    super::mob::MobKind::CopperGolem(_) => "CopperGolem",
                    super::mob::MobKind::Allay(_) => "Allay",
                    super::mob::MobKind::SnowGolem(_) => "SnowGolem",
                    super::mob::MobKind::Cod(_) => "Cod",
                    super::mob::MobKind::Salmon(_) => "Salmon",
                    super::mob::MobKind::Pufferfish(_) => "Pufferfish",
                    super::mob::MobKind::TropicalFish(_) => "TropicalFish",
                    super::mob::MobKind::Cat(_) => "Cat",
                    super::mob::MobKind::Parrot(_) => "Parrot",
                    super::mob::MobKind::Bee(_) => "Bee",
                    super::mob::MobKind::Shulker(_) => "Shulker",
                    super::mob::MobKind::Wither(_) => "WitherBoss",
                    super::mob::MobKind::EnderDragon(_) => "EnderDragon",
                    super::mob::MobKind::Ocelot(_) => "Ocelot",
                    super::mob::MobKind::Horse(_) => "Horse",
                    super::mob::MobKind::Donkey(_) => "Donkey",
                    super::mob::MobKind::Mule(_) => "Mule",
                    super::mob::MobKind::SkeletonHorse(_) => "SkeletonHorse",
                    super::mob::MobKind::ZombieHorse(_) => "ZombieHorse",
                    super::mob::MobKind::Llama(_) => "Llama",
                    super::mob::MobKind::TraderLlama(_) => "TraderLlama",
                    super::mob::MobKind::Camel(_) => "Camel",
                    super::mob::MobKind::CamelHusk(_) => "CamelHusk",
                    super::mob::MobKind::Nautilus(_) => "Nautilus",
                    super::mob::MobKind::ZombieNautilus(_) => "ZombieNautilus",
                    super::mob::MobKind::HappyGhast(_) => "HappyGhast",
                    super::mob::MobKind::WanderingTrader(_) => "WanderingTrader",
                    super::mob::MobKind::Axolotl(_) => "Axolotl",
                    super::mob::MobKind::Frog(_) => "Frog",
                    super::mob::MobKind::Tadpole(_) => "Tadpole",
                    super::mob::MobKind::Villager(_) => "Villager",
                    super::mob::MobKind::Fox(_) => "Fox",
                    super::mob::MobKind::Rabbit(_) => "Rabbit",
                    super::mob::MobKind::Turtle(_) => "Turtle",
                },
                Some(LevelEntity::Projectile(projectile)) => match projectile.kind {
                    ProjectileKind::Snowball(_) => "Snowball",
                    ProjectileKind::ThrownEgg(_) => "ThrownEgg",
                    ProjectileKind::ThrownEnderpearl(_) => "ThrownEnderpearl",
                    ProjectileKind::ThrownExperienceBottle(_) => "ThrownExperienceBottle",
                    ProjectileKind::ThrownSplashPotion(_) => "ThrownSplashPotion",
                    ProjectileKind::ThrownLingeringPotion(_) => "ThrownLingeringPotion",
                    ProjectileKind::Arrow(_) => "Arrow",
                    ProjectileKind::SpectralArrow(_) => "SpectralArrow",
                    ProjectileKind::ThrownTrident(_) => "ThrownTrident",
                    ProjectileKind::FireworkRocket(_) => "FireworkRocketEntity",
                    ProjectileKind::SmallFireball(_) => "SmallFireball",
                    ProjectileKind::LlamaSpit(_) => "LlamaSpit",
                    ProjectileKind::LargeFireball(_) => "LargeFireball",
                    ProjectileKind::WindCharge(_) => "WindCharge",
                    ProjectileKind::FishingHook(_) => "FishingHook",
                    ProjectileKind::ShulkerBullet(_) => "ShulkerBullet",
                    ProjectileKind::WitherSkull(_) => "WitherSkull",
                    ProjectileKind::DragonFireball(_) => "DragonFireball",
                    ProjectileKind::BreezeWindCharge(_) => "BreezeWindCharge",
                },
                Some(LevelEntity::AreaEffectCloud(_)) => "AreaEffectCloud",
                Some(LevelEntity::Minecart(minecart)) => match minecart.kind {
                    super::vehicle::minecart::abstract_minecart::MinecartKind::Minecart(_) => "Minecart",
                    super::vehicle::minecart::abstract_minecart::MinecartKind::Chest(_) => "MinecartChest",
                    super::vehicle::minecart::abstract_minecart::MinecartKind::Furnace(_) => "MinecartFurnace",
                    super::vehicle::minecart::abstract_minecart::MinecartKind::Hopper(_) => "MinecartHopper",
                    super::vehicle::minecart::abstract_minecart::MinecartKind::Tnt(_) => "MinecartTNT",
                    super::vehicle::minecart::abstract_minecart::MinecartKind::Spawner(_) => "MinecartSpawner",
                    super::vehicle::minecart::abstract_minecart::MinecartKind::CommandBlock(_) => "MinecartCommandBlock",
                },
                Some(LevelEntity::Boat(boat)) => match boat.kind {
                    super::vehicle::boat::abstract_boat::BoatKind::Boat => "Boat",
                    super::vehicle::boat::abstract_boat::BoatKind::Raft => "Raft",
                    super::vehicle::boat::abstract_boat::BoatKind::ChestBoat => "ChestBoat",
                    super::vehicle::boat::abstract_boat::BoatKind::ChestRaft => "ChestRaft",
                },
                _ => "Entity",
            };
            (class, name)
        };
        format!("{class}['{name}'/{id}, l='ServerLevel[{}]', x={:.2}, y={:.2}, z={:.2}]", self.level_name, e.x(), e.y(), e.z())
    }

    pub fn spawn_at_location(&mut self, id: EntityId, stack: ItemStack, offset: f32) -> Option<EntityId> {
        self.spawn_at_location_with_offset(id, stack, Vec3::new(0.0, offset as f64, 0.0))
    }

    pub fn spawn_at_location_with_offset(&mut self, id: EntityId, stack: ItemStack, offset: Vec3) -> Option<EntityId> {
        if stack.is_empty() {
            return None;
        }
        let e = self.entity(id);
        let pos = Vec3::new(e.x() + offset.x, e.y() + offset.y, e.z() + offset.z);
        let mut item = ItemEntity::new(pos, stack);
        item.set_default_pickup_delay();
        let item_id = item.entity.id;
        self.add_fresh_entity(LevelEntity::Item(item));
        Some(item_id)
    }
}

fn collect_candidate_step_up_heights(bounding_box: &AABB, colliders: &[Collider], max_step_height: f32, step_height_to_skip: f32) -> Vec<f32> {
    let mut candidates: Vec<f32> = Vec::with_capacity(4);
    for collider in colliders {
        for coord in collider.coords(Axis::Y) {
            let relative = (coord - bounding_box.min_y) as f32;
            if !(relative < 0.0) && relative != step_height_to_skip {
                if relative > max_step_height {
                    break;
                }
                if !candidates.iter().any(|c| c.to_bits() == relative.to_bits()) {
                    candidates.push(relative);
                }
            }
        }
    }
    candidates.sort_by(f32::total_cmp);
    candidates
}

trait SignumJava {
    fn signum_java(self) -> f64;
}

impl SignumJava for f64 {
    fn signum_java(self) -> f64 {
        if self == 0.0 || self.is_nan() { self } else { self.signum() }
    }
}

impl ServerLevel {
    pub fn teleport_in_level(&mut self, id: EntityId, transition: TeleportTransition) -> Option<EntityId> {
        if self.entity_ref(id).is_none_or(Entity::is_removed) {
            return None;
        }
        if self.is_player(id) {
            return Some(self.server_player_teleport_same_dimension(id, transition));
        }
        if !transition.as_passenger {
            self.stop_riding(id);
        }
        Some(self.teleport_same_dimension(id, transition))
    }

    pub fn teleport_same_dimension(&mut self, id: EntityId, transition: TeleportTransition) -> EntityId {
        for passenger in self.passengers(id).to_vec() {
            let passenger_transition = self.calculate_passenger_transition(id, &transition, passenger);
            self.teleport_in_level(passenger, passenger_transition);
        }
        let e = self.entity_mut(id);
        let current = relative::PositionMoveRotation { position: e.position(), delta_movement: e.delta_movement, y_rot: e.y_rot, x_rot: e.x_rot };
        e.teleport_set_position(current, transition.position_move_rotation(), transition.relatives);
        self.entity_moved(id);
        if !transition.as_passenger {
            self.send_teleport_transition_to_players(id, &transition);
        }
        transition.post_teleport_transition.on_transition(self, id);
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Item(_)) => self.merge_with_neighbours(id),
            Some(LevelEntity::PrimedTnt(tnt)) => tnt.used_portal = true,
            _ => {}
        }
        if self.is_thrown_enderpearl(id) {
            self.thrown_enderpearl_teleported(id);
        }
        id
    }

    pub fn calculate_passenger_transition(&self, id: EntityId, transition: &TeleportTransition, passenger: EntityId) -> TeleportTransition {
        let e = self.entity(id);
        calculate_passenger_transition(e.position(), e.y_rot, e.x_rot, transition, self.entity(passenger))
    }

    fn send_teleport_transition_to_players(&mut self, id: EntityId, transition: &TeleportTransition) {
        use crate::network::connection::encode;
        use wafer_data::packets::play::clientbound::TELEPORT_ENTITY;
        let controller = self.controlling_passenger(id);
        let passengers = self.indirect_passengers(id);
        let e = self.entity(id);
        let own = relative::PositionMoveRotation { position: e.position(), delta_movement: e.delta_movement, y_rot: e.y_rot, x_rot: e.x_rot };
        let teleport = |values: relative::PositionMoveRotation, relatives: u16| {
            encode(TELEPORT_ENTITY, |w| {
                w.varint(id);
                values.write(w);
                w.i32(relatives as i32);
                w.bool(e.on_ground);
            })
        };
        for &passenger in &passengers {
            if let Some(player) = self.players.get(&passenger) {
                let packet = if controller == Some(passenger) { teleport(transition.position_move_rotation(), transition.relatives) } else { teleport(own, 0) };
                player.connection.send_raw(packet);
            }
        }
        let has_update_interval = self.entities.get(&id).is_none_or(|entity| entity.tracking_config().update_interval != crate::server::level::server_entity::UpdateInterval::Never);
        let server_and_client = matches!(self.entities.get(&id), Some(LevelEntity::ExperienceOrb(_) | LevelEntity::FallingBlock(_)));
        if !has_update_interval || server_and_client {
            let packet = teleport(own, 0);
            if let Some(tracked) = self.chunk_map.tracked.get(id) {
                for viewer in &tracked.seen_by {
                    if !passengers.contains(viewer)
                        && let Some(p) = self.players.get(viewer)
                    {
                        p.connection.send_raw(packet.clone());
                    }
                }
            }
        }
    }

    fn remove_after_changing_dimensions(&mut self, id: EntityId) {
        self.entity_mut(id).removal = Some(RemovalReason::ChangedDimension);
        if self.is_leashable(id) {
            self.remove_leash(id);
        }
        if self.is_living(id) {
            self.get_waypoint_manager().untrack_waypoint(self, id);
        }
        self.remove_entity(id);
    }

    pub fn set_portal_cooldown(&mut self, id: EntityId) {
        let delay = self.get_dimension_changing_delay(id);
        self.entity_mut(id).portal_cooldown = delay;
    }

    pub fn is_on_portal_cooldown(&self, id: EntityId) -> bool {
        self.entity(id).portal_cooldown > 0
    }

    fn process_portal_cooldown(&mut self, id: EntityId) {
        if self.is_on_portal_cooldown(id) {
            self.entity_mut(id).portal_cooldown -= 1;
        }
    }

    pub fn can_use_portal(&self, id: EntityId, ignore_passenger: bool) -> bool {
        if self.is_wither(id) || self.is_ender_dragon(id) {
            return false;
        }
        if self.is_creaking(id) && !self.creaking_can_use_portal(id) {
            return false;
        }
        if self.as_projectile(id).is_some_and(super::projectile::projectile::Projectile::is_throwable_item_projectile) {
            return true;
        }
        if self.is_fishing_hook(id) {
            return false;
        }
        (ignore_passenger || self.vehicle(id).is_none()) && self.is_alive(id) && !(self.is_living(id) && self.is_sleeping(id))
    }

    pub fn can_teleport(&self, id: EntityId, from: &str, to: &str) -> bool {
        if self.is_thrown_enderpearl(id)
            && from == crate::server::THE_END
            && to == crate::server::OVERWORLD
            && let Some(seen_credits) = self.thrown_enderpearl_can_teleport(id)
        {
            return seen_credits;
        }
        true
    }

    pub fn is_allowed_to_enter_portal(&self, to_dimension: &str) -> bool {
        to_dimension != crate::server::THE_NETHER || self.game_rules.get_bool(wafer_data::game_rule::ALLOW_ENTERING_NETHER_USING_PORTALS)
    }

    pub fn set_as_inside_portal(&mut self, id: EntityId, portal: Portal, pos: BlockPos) {
        if self.is_on_portal_cooldown(id) {
            self.set_portal_cooldown(id);
            return;
        }
        let e = self.entity_mut(id);
        match &mut e.portal_process {
            Some(process) if process.is_same_portal(portal) => {
                if !process.is_inside_portal_this_tick() {
                    process.update_entry_position(pos);
                    process.set_as_inside_portal_this_tick(true);
                }
            }
            _ => e.portal_process = Some(PortalProcessor::new(portal, pos)),
        }
    }

    pub fn handle_portal(&mut self, id: EntityId) {
        self.process_portal_cooldown(id);
        let Some(mut process) = self.entity(id).portal_process else { return };
        let allowed_to_teleport = self.can_use_portal(id, false);
        let portal = process.portal();
        let teleport = process.process_portal_teleportation(allowed_to_teleport, || portal.get_portal_transition_time(self, id));
        self.entity_mut(id).portal_process = Some(process);
        if teleport {
            self.set_portal_cooldown(id);
            if portal == Portal::EndGateway && !self.is_player(id) {
                if let Some(transition) = crate::world::level::block::end_gateway_block::get_portal_destination_in_level(self, process.get_entry_position()) {
                    self.teleport_same_dimension(id, transition);
                }
                return;
            }
            let pending = self.capture_portal_teleport(id, portal, process.get_entry_position());
            self.pending_portal_teleport = Some(pending);
        } else if process.has_expired() {
            self.entity_mut(id).portal_process = None;
        }
    }

    fn capture_portal_teleport(&mut self, id: EntityId, portal: Portal, entry_position: BlockPos) -> PendingPortalTeleport {
        let entity = self.portal_entity(id);
        let destination = portal.get_destination_dimension(self.dimension);
        let removed = entity.data.is_some() && destination.is_none_or(|destination| destination != self.dimension && self.is_allowed_to_enter_portal(destination));
        if removed {
            self.entity_mut(id).removal = Some(RemovalReason::ChangedDimension);
            let from_or_to_end = self.dimension == crate::server::THE_END || destination == Some(crate::server::THE_END);
            if from_or_to_end && let Some(LevelEntity::FallingBlock(falling)) = self.entities.get_mut(&id) {
                falling.force_tick_after_teleport_to_duplicate = true;
            }
        }
        PendingPortalTeleport { entity, portal, entry_position, removed }
    }

    pub fn portal_entity(&self, id: EntityId) -> PortalEntity {
        let data = self.entities.get(&id).map(|old| {
            let mut data = Vec::new();
            old.save_without_id(&mut data);
            data
        });
        let e = self.entity(id);
        PortalEntity {
            id,
            entity_type: e.entity_type,
            position: e.position(),
            delta_movement: e.delta_movement,
            y_rot: e.y_rot,
            x_rot: e.x_rot,
            dimensions: e.dimensions,
            collision_context: CollisionContext::of(self, id),
            is_spectator: self.is_spectator(id),
            portal_cooldown: e.portal_cooldown,
            portal_process: e.portal_process,
            data,
        }
    }
}

impl crate::server::Server {
    pub fn teleport_to(&mut self, id: EntityId, level: usize, pos: Vec3, relatives: u16, y_rot: f32, x_rot: f32, reset_camera: bool) -> bool {
        if self.has_player(id) {
            return self.server_player_teleport_to(id, level, pos, relatives, y_rot, x_rot, reset_camera);
        }
        let transition = TeleportTransition::with_relatives(level, pos, Vec3::ZERO, y_rot, x_rot, relatives, PostTeleportTransition::DO_NOTHING);
        self.teleport(id, transition).is_some()
    }

    pub fn teleport(&mut self, id: EntityId, transition: TeleportTransition) -> Option<EntityId> {
        if self.has_player(id) {
            return self.server_player_teleport(id, transition);
        }
        let old_level = self.level_of(id)?;
        if self.levels[old_level].entity(id).is_removed() {
            return None;
        }
        if transition.new_level != old_level {
            if !transition.as_passenger {
                self.levels[old_level].stop_riding(id);
            }
            let old_entity = self.levels[old_level].portal_entity(id);
            self.teleport_cross_dimension(old_level, &old_entity, transition)
        } else {
            self.levels[old_level].teleport_in_level(id, transition)
        }
    }

    fn teleport_cross_dimension(&mut self, old_level: usize, old_entity: &PortalEntity, transition: TeleportTransition) -> Option<EntityId> {
        let level = &mut self.levels[old_level];
        let old_passengers = if level.entities.contains_key(&old_entity.id) { level.passengers(old_entity.id).to_vec() } else { Vec::new() };
        if !old_passengers.is_empty() {
            level.eject_passengers(old_entity.id);
        }
        let mut new_passengers = Vec::with_capacity(old_passengers.len());
        for passenger in old_passengers {
            let passenger_transition = calculate_passenger_transition(old_entity.position, old_entity.y_rot, old_entity.x_rot, &transition, self.levels[old_level].entity(passenger));
            if let Some(new_passenger) = self.teleport(passenger, passenger_transition) {
                new_passengers.push(new_passenger);
            }
        }
        let mut new_entity = ServerLevel::create_entity(registry::ENTITY_TYPE[old_entity.entity_type as usize])?;
        let saved = Nbt::Compound(old_entity.data.clone().unwrap_or_default());
        let _ = new_entity.load(&saved);
        let new_entity = self.levels[transition.new_level].reassess_weapon_goal_detached(new_entity);
        let mut new_entity = self.levels[transition.new_level].read_level_dependent_save_data_detached(new_entity, &saved);
        let e = new_entity.entity_mut();
        e.portal_cooldown = old_entity.portal_cooldown;
        e.portal_process = old_entity.portal_process;
        if self.levels[old_level].entities.contains_key(&old_entity.id) {
            self.levels[old_level].remove_after_changing_dimensions(old_entity.id);
        }
        let current = relative::PositionMoveRotation {
            position: old_entity.position,
            delta_movement: old_entity.delta_movement,
            y_rot: old_entity.y_rot,
            x_rot: old_entity.x_rot,
        };
        new_entity.entity_mut().teleport_set_position(current, transition.position_move_rotation(), transition.relatives);
        let is_item = matches!(new_entity, LevelEntity::Item(_));
        if let LevelEntity::PrimedTnt(tnt) = &mut new_entity {
            tnt.used_portal = true;
        }
        let new_id = new_entity.entity().id;
        let level = &mut self.levels[transition.new_level];
        level.add_entity(new_entity);
        for new_passenger in new_passengers {
            level.start_riding(new_passenger, new_id, true, false);
        }
        level.reset_empty_time();
        transition.post_teleport_transition.on_transition(level, new_id);
        if is_item {
            level.merge_with_neighbours(new_id);
        }
        self.teleport_spectators(old_entity.id, transition, old_level);
        if self.levels[transition.new_level].is_thrown_enderpearl(new_id) {
            self.levels[transition.new_level].thrown_enderpearl_teleported(new_id);
        }
        Some(new_id)
    }

    pub fn handle_pending_portal_teleport(&mut self, level: usize) {
        let Some(pending) = self.levels[level].pending_portal_teleport.take() else { return };
        let transition = pending.portal.get_portal_destination(self, level, &pending.entity, pending.entry_position);
        let teleported = transition.is_some_and(|transition| self.teleport_to_portal_destination(level, &pending, transition));
        if !teleported
            && pending.removed
            && let Some(old) = self.levels[level].entities.get_mut(&pending.entity.id)
            && old.entity().removal == Some(RemovalReason::ChangedDimension)
        {
            old.entity_mut().removal = None;
        }
    }

    pub fn handle_pending_spectator_portal_use(&mut self, level: usize) {
        let Some(pending) = self.levels[level].pending_spectator_portal_use.take() else { return };
        let entity = self.levels[level].portal_entity(pending.player);
        match pending.portal.get_portal_destination(self, level, &entity, pending.pos) {
            Some(transition) => {
                if self.levels[level].get_camera(pending.player) != pending.player {
                    self.set_camera(pending.player, None);
                }
                let portal_teleport = PendingPortalTeleport { entity, portal: pending.portal, entry_position: pending.pos, removed: false };
                self.teleport_to_portal_destination(level, &portal_teleport, transition);
            }
            None => {
                let message = crate::network::chat::Component::translatable("spectator.cannot_teleport").with_color(crate::network::chat::ChatFormatting::Red);
                self.levels[level].player(pending.player).send_overlay_message(&message);
            }
        }
        if let Some(player) = self.get_player(pending.player) {
            for packet in pending.packets {
                player.connection.send_raw(packet);
            }
        }
    }

    fn teleport_to_portal_destination(&mut self, level: usize, pending: &PendingPortalTeleport, transition: TeleportTransition) -> bool {
        let id = pending.entity.id;
        let (from, to) = (self.levels[level].dimension, self.levels[transition.new_level].dimension);
        let current_level = &self.levels[level];
        if !current_level.is_allowed_to_enter_portal(to) || from != to && !current_level.can_teleport(id, from, to) {
            return false;
        }
        if self.has_player(id) {
            self.server_player_teleport(id, transition);
        } else if transition.new_level != level {
            self.teleport_cross_dimension(level, &pending.entity, transition);
        } else {
            self.levels[level].teleport_same_dimension(id, transition);
        }
        true
    }
}

pub fn calculate_passenger_transition(position: Vec3, y_rot: f32, x_rot: f32, transition: &TeleportTransition, passenger: &Entity) -> TeleportTransition {
    use relative::relative::{X, X_ROT, Y, Y_ROT, Z};
    let has = |flag: u16| transition.relatives & flag != 0;
    let passenger_y_rot = transition.y_rot + if has(Y_ROT) { 0.0 } else { passenger.y_rot - y_rot };
    let passenger_x_rot = transition.x_rot + if has(X_ROT) { 0.0 } else { passenger.x_rot - x_rot };
    let passenger_offset = passenger.position().subtract(position);
    let passenger_pos = transition.position.add(
        if has(X) { 0.0 } else { passenger_offset.x },
        if has(Y) { 0.0 } else { passenger_offset.y },
        if has(Z) { 0.0 } else { passenger_offset.z },
    );
    transition.with_position(passenger_pos).with_rotation(passenger_y_rot, passenger_x_rot).transition_as_passenger()
}

pub fn set_passengers_packet(vehicle: EntityId, passengers: &[EntityId]) -> Vec<u8> {
    crate::network::connection::encode(wafer_data::packets::play::clientbound::SET_PASSENGERS, |w| {
        w.varint(vehicle);
        w.varint(passengers.len() as i32);
        for &passenger in passengers {
            w.varint(passenger);
        }
    })
}

pub fn get_collision_horizontal_escape_vector(collider_width: f64, colliding_width: f64, direction_degrees: f32) -> Vec3 {
    let distance = (collider_width + colliding_width + 1.0e-5f32 as f64) / 2.0;
    let direction_x = -mth::sin((direction_degrees * mth::DEG_TO_RAD) as f64);
    let direction_z = mth::cos((direction_degrees * mth::DEG_TO_RAD) as f64);
    let scale = direction_x.abs().max(direction_z.abs());
    Vec3::new(direction_x as f64 * distance / scale as f64, 0.0, direction_z as f64 * distance / scale as f64)
}

impl ServerLevel {
    pub fn vehicle(&self, id: EntityId) -> Option<EntityId> {
        self.entity_ref(id).and_then(|e| e.vehicle)
    }

    pub fn passengers(&self, id: EntityId) -> &[EntityId] {
        self.entity_ref(id).map_or(&[], |e| &e.passengers)
    }

    pub fn is_passenger(&self, id: EntityId) -> bool {
        self.vehicle(id).is_some()
    }

    pub fn is_vehicle(&self, id: EntityId) -> bool {
        !self.passengers(id).is_empty()
    }

    pub fn first_passenger(&self, id: EntityId) -> Option<EntityId> {
        self.passengers(id).first().copied()
    }

    pub fn has_passenger(&self, id: EntityId, entity: EntityId) -> bool {
        self.passengers(id).contains(&entity)
    }

    fn collect_self_and_passengers(&self, id: EntityId, out: &mut Vec<EntityId>) {
        out.push(id);
        for &passenger in self.passengers(id) {
            self.collect_self_and_passengers(passenger, out);
        }
    }

    fn collect_passengers_and_self(&self, id: EntityId, out: &mut Vec<EntityId>) {
        for &passenger in self.passengers(id) {
            self.collect_passengers_and_self(passenger, out);
        }
        out.push(id);
    }

    pub fn indirect_passengers(&self, id: EntityId) -> Vec<EntityId> {
        let mut out = Vec::new();
        for &passenger in self.passengers(id) {
            self.collect_self_and_passengers(passenger, &mut out);
        }
        out
    }

    pub fn self_and_passengers(&self, id: EntityId) -> Vec<EntityId> {
        let mut out = Vec::new();
        self.collect_self_and_passengers(id, &mut out);
        out
    }

    pub fn passengers_and_self(&self, id: EntityId) -> Vec<EntityId> {
        let mut out = Vec::new();
        self.collect_passengers_and_self(id, &mut out);
        out
    }

    pub fn count_player_passengers(&self, id: EntityId) -> usize {
        self.indirect_passengers(id).into_iter().filter(|&p| self.is_player(p)).count()
    }

    pub fn has_exactly_one_player_passenger(&self, id: EntityId) -> bool {
        self.count_player_passengers(id) == 1
    }

    pub fn root_vehicle(&self, id: EntityId) -> EntityId {
        let mut result = id;
        while let Some(vehicle) = self.vehicle(result) {
            result = vehicle;
        }
        result
    }

    pub fn is_passenger_of_same_vehicle(&self, id: EntityId, other: EntityId) -> bool {
        self.root_vehicle(id) == self.root_vehicle(other)
    }

    pub fn has_indirect_passenger(&self, id: EntityId, entity: EntityId) -> bool {
        match self.vehicle(entity) {
            None => false,
            Some(ridden) => ridden == id || self.has_indirect_passenger(id, ridden),
        }
    }

    pub fn controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_)) => self.abstract_boat_controlling_passenger(id),
            Some(LevelEntity::Mob(mob)) if mob.abstract_horse().is_some() => self.abstract_horse_get_controlling_passenger(id),
            Some(LevelEntity::Mob(mob)) if mob.abstract_nautilus().is_some() => self.abstract_nautilus_get_controlling_passenger(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::HappyGhast(_)) => self.happy_ghast_get_controlling_passenger(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Pig(_)) => self.pig_get_controlling_passenger(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Strider(_)) => self.strider_get_controlling_passenger(id),
            Some(LevelEntity::Mob(_)) => self.mob_get_controlling_passenger(id),
            _ => None,
        }
    }

    pub fn mob_get_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        let first_passenger = self.first_passenger(id)?;
        (!self.mob(id)?.is_no_ai() && self.is_mob(first_passenger) && self.can_control_vehicle(first_passenger)).then_some(first_passenger)
    }

    pub fn has_controlling_passenger(&self, id: EntityId) -> bool {
        self.controlling_passenger(id).is_some()
    }

    pub fn controlled_vehicle(&self, id: EntityId) -> Option<EntityId> {
        self.vehicle(id).filter(|&vehicle| self.controlling_passenger(vehicle) == Some(id))
    }

    pub fn can_control_vehicle(&self, id: EntityId) -> bool {
        !entity_type_tags::NON_CONTROLLING_RIDER.contains(self.entity(id).entity_type as usize)
    }

    pub fn dismounts_underwater(&self, id: EntityId) -> bool {
        entity_type_tags::DISMOUNTS_UNDERWATER.contains(self.entity(id).entity_type as usize)
    }

    fn can_serialize(&self, id: EntityId) -> bool {
        !self.is_player(id) && !matches!(self.entities.get(&id), Some(LevelEntity::LightningBolt(_)))
    }

    pub fn start_riding(&mut self, id: EntityId, entity_to_ride: EntityId, force: bool, send_event_and_triggers: bool) -> bool {
        if self.is_shulker(id) {
            self.shulker_start_riding(id);
        }
        let entity_to_ride = if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Piglin(_))) { self.piglin_get_entity_to_ride(id, entity_to_ride) } else { entity_to_ride };
        if self.vehicle(id) == Some(entity_to_ride) {
            return false;
        }
        if !self.could_accept_passenger(entity_to_ride) {
            return false;
        }
        if !self.can_serialize(entity_to_ride) {
            return false;
        }
        let mut vehicle_entity = entity_to_ride;
        while let Some(vehicle) = self.vehicle(vehicle_entity) {
            if vehicle == id {
                return false;
            }
            vehicle_entity = vehicle;
        }
        if !(force || self.can_ride(id, entity_to_ride) && self.can_add_passenger(entity_to_ride, id)) {
            return false;
        }
        if self.is_passenger(id) {
            self.stop_riding(id);
        }
        self.set_entity_pose(id, data::Pose::Standing);
        self.entity_mut(id).vehicle = Some(entity_to_ride);
        self.add_passenger(entity_to_ride, id);
        if send_event_and_triggers {
            let vehicle_position = self.entity(entity_to_ride).position();
            self.game_event_from_entity(Some(id), game_event::ENTITY_MOUNT, vehicle_position);
            for passenger in self.indirect_passengers(entity_to_ride) {
                if self.is_player(passenger) {
                    crate::advancements::triggers::criteria_triggers::START_RIDING_TRIGGER.trigger(self, passenger);
                }
            }
        }
        if self.is_player(id) {
            self.position_rider(entity_to_ride, id);
            let position = self.entity(id).position();
            let destination = relative::PositionMoveRotation { position, delta_movement: Vec3::ZERO, y_rot: 0.0, x_rot: 0.0 };
            self.player_mut(id).teleport_relative(destination, relative::relative::ROTATION);
            if self.is_living(entity_to_ride) {
                self.send_active_effects(entity_to_ride, id);
            }
            let packet = set_passengers_packet(entity_to_ride, self.passengers(entity_to_ride));
            self.player(id).connection.send_raw(packet);
        }
        if self.is_mob(id) && self.is_leashed(id) {
            self.drop_leash(id);
        }
        true
    }

    fn can_ride(&self, id: EntityId, _vehicle: EntityId) -> bool {
        if self.is_wither(id) || self.is_warden(id) || self.is_ender_dragon(id) {
            return false;
        }
        let e = self.entity(id);
        !e.is_shift_key_down() && e.boarding_cooldown <= 0
    }

    pub fn can_add_passenger(&self, id: EntityId, _passenger: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_)) => self.abstract_boat_can_add_passenger(id),
            Some(LevelEntity::Marker(_)) => super::marker::Marker::can_add_passenger(),
            Some(LevelEntity::OminousItemSpawner(_)) => super::ominous_item_spawner::OminousItemSpawner::can_add_passenger(),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Strider(_)) => self.strider_can_add_passenger(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Camel(_) | super::mob::MobKind::CamelHusk(_)) => self.camel_can_add_passenger(id),
            Some(LevelEntity::Mob(mob)) if mob.abstract_nautilus().is_some() => self.abstract_nautilus_can_add_passenger(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::HappyGhast(_)) => self.happy_ghast_can_add_passenger(id),
            _ => self.passengers(id).is_empty(),
        }
    }

    fn could_accept_passenger(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Marker(_)) => super::marker::Marker::could_accept_passenger(),
            Some(LevelEntity::OminousItemSpawner(_)) => super::ominous_item_spawner::OminousItemSpawner::could_accept_passenger(),
            _ => true,
        }
    }

    fn add_passenger(&mut self, id: EntityId, passenger: EntityId) {
        if self.is_abstract_horse(id) {
            self.abstract_horse_add_passenger(id, passenger);
        } else if self.is_happy_ghast(id) {
            self.happy_ghast_add_passenger(id, passenger);
        } else {
            self.entity_add_passenger(id, passenger);
        }
    }

    pub fn entity_add_passenger(&mut self, id: EntityId, passenger: EntityId) {
        let player_first = self.is_player(passenger) && !self.first_passenger(id).is_some_and(|first| self.is_player(first));
        let passengers = &mut self.entity_mut(id).passengers;
        if !passengers.is_empty() && player_first {
            passengers.insert(0, passenger);
        } else {
            passengers.push(passenger);
        }
    }

    fn remove_passenger(&mut self, id: EntityId, passenger: EntityId) {
        self.entity_mut(id).passengers.retain(|&p| p != passenger);
        self.entity_mut(passenger).boarding_cooldown = 60;
        if self.is_cushion(id) {
            self.cushion_remove_passenger(id);
        }
        if self.is_happy_ghast(id) {
            self.happy_ghast_remove_passenger(id);
        }
    }

    pub fn eject_passengers(&mut self, id: EntityId) {
        let mut i = self.passengers(id).len();
        while i > 0 {
            i -= 1;
            if let Some(&passenger) = self.passengers(id).get(i) {
                self.stop_riding(passenger);
            }
        }
    }

    pub fn un_ride(&mut self, id: EntityId) {
        if self.is_vehicle(id) {
            self.eject_passengers(id);
        }
        if self.is_passenger(id) {
            self.stop_riding(id);
        }
    }

    pub fn remove_vehicle(&mut self, id: EntityId) {
        let old_vehicle = self.vehicle(id);
        if let Some(old) = old_vehicle {
            self.entity_mut(id).vehicle = None;
            self.remove_passenger(old, id);
            let removal_reason = self.entity(id).removal;
            if removal_reason.is_none_or(RemovalReason::should_destroy)
                && let Some(old_position) = self.entity_ref(old).map(Entity::position)
            {
                self.game_event_from_entity(Some(id), game_event::ENTITY_DISMOUNT, old_position);
            }
        }
        if self.is_player(id) {
            self.entity_mut(id).boarding_cooldown = 0;
            if let Some(old) = old_vehicle.filter(|&old| self.entity_ref(old).is_some()) {
                if self.is_living(old) {
                    let effects: Vec<_> = self.living(old).active_effects.iter().map(|effect| effect.effect).collect();
                    for effect in effects {
                        self.player(id).connection.send_raw(super::living_entity::remove_mob_effect(old, effect));
                    }
                }
                let packet = set_passengers_packet(old, self.passengers(old));
                self.player(id).connection.send_raw(packet);
            }
        }
    }

    pub fn stop_riding(&mut self, id: EntityId) {
        let old_vehicle = self.vehicle(id);
        self.remove_vehicle(id);
        if self.is_living(id)
            && let Some(old) = old_vehicle
            && self.vehicle(id) != Some(old)
        {
            self.dismount_vehicle(id, old);
        }
        if self.is_shulker(id) {
            self.shulker_stop_riding(id);
        }
    }

    pub fn set_removed(&mut self, id: EntityId, reason: RemovalReason) {
        let e = self.entity_mut(id);
        let reason = *e.removal.get_or_insert(reason);
        if reason.should_destroy() {
            self.stop_riding(id);
        }
        for passenger in self.passengers(id).to_vec() {
            self.stop_riding(passenger);
        }
        if !self.is_player(id) && self.entity_manager.has_callback(id) {
            self.entity_removed(id, Some(reason));
            if self.as_projectile(id).is_some_and(|projectile| matches!(projectile.kind, crate::world::entity::projectile::projectile::ProjectileKind::ThrownEnderpearl(_))) {
                self.thrown_enderpearl_on_removal(id, Some(reason));
            }
        }
    }

    fn dismount_vehicle(&mut self, id: EntityId, vehicle: EntityId) {
        let vehicle_removed = self.entity_ref(vehicle).is_none_or(Entity::is_removed);
        let teleport_target = if self.entity(id).is_removed() {
            self.entity(id).position()
        } else if !vehicle_removed && !{
            let pos = self.entity(vehicle).block_position();
            self.block_state(pos).is(&block_tags::PORTALS)
        } {
            self.dismount_location_for_passenger(vehicle, id)
        } else {
            let e = self.entity(id);
            let vehicle_y = self.entity_ref(vehicle).map_or(e.y(), Entity::y);
            let max_y = e.y().max(vehicle_y);
            let mut teleport_target = Vec3::new(e.x(), max_y, e.z());
            let (width, height) = (e.dimensions.width, e.dimensions.height);
            let is_small = width <= 4.0 && height <= 4.0;
            if is_small {
                let half_height = height as f64 / 2.0;
                let center = teleport_target.add(0.0, half_height, 0.0);
                let allowed_centers = AABB::of_size(center, width as f64, height as f64, width as f64);
                let context = CollisionContext::of(self, id);
                teleport_target = self
                    .find_free_position(&context, &allowed_centers, center, width as f64, height as f64, width as f64)
                    .map_or(teleport_target, |pos| pos.add(0.0, -half_height, 0.0));
            }
            teleport_target
        };
        self.dismount_to(id, teleport_target.x, teleport_target.y, teleport_target.z);
    }

    pub fn dismount_to(&mut self, id: EntityId, x: f64, y: f64, z: f64) {
        self.teleport_to(id, x, y, z);
    }

    pub fn teleport_to(&mut self, id: EntityId, x: f64, y: f64, z: f64) {
        if self.is_player(id) {
            let destination = relative::PositionMoveRotation { position: Vec3::new(x, y, z), delta_movement: Vec3::ZERO, y_rot: 0.0, x_rot: 0.0 };
            self.player_mut(id).teleport_relative(destination, relative::relative::DELTA | relative::relative::ROTATION);
            self.entity_moved(id);
            return;
        }
        let e = self.entity_mut(id);
        let (y_rot, x_rot) = (e.y_rot, e.x_rot);
        e.snap_to(x, y, z, y_rot, x_rot);
        self.entity_moved(id);
        self.teleport_passengers(id);
    }

    fn teleport_passengers(&mut self, id: EntityId) {
        for entity in self.self_and_passengers(id) {
            for passenger in self.passengers(entity).to_vec() {
                self.position_rider_with(entity, passenger, true);
            }
        }
    }

    pub fn dismount_location_for_passenger(&mut self, id: EntityId, passenger: EntityId) -> Vec3 {
        match self.entities.get(&id) {
            Some(LevelEntity::Minecart(_)) => self.abstract_minecart_dismount_location_for_passenger(id, passenger),
            Some(LevelEntity::Boat(_)) => self.abstract_boat_dismount_location_for_passenger(id, passenger),
            Some(LevelEntity::Mob(mob)) if mob.abstract_horse().is_some() => self.abstract_horse_dismount_location_for_passenger(id, passenger),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::HappyGhast(_)) => self.happy_ghast_dismount_location_for_passenger(id),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Strider(_)) => self.strider_dismount_location_for_passenger(id, passenger),
            Some(LevelEntity::Mob(mob)) if mob.animal().is_some() => self.animal_dismount_location_for_passenger(id, passenger),
            _ => self.entity_dismount_location_for_passenger(id, passenger),
        }
    }

    pub fn entity_dismount_location_for_passenger(&self, id: EntityId, _passenger: EntityId) -> Vec3 {
        let e = self.entity(id);
        Vec3::new(e.x(), e.bounding_box().max_y, e.z())
    }

    pub fn dismount_poses(&self, id: EntityId) -> &'static [data::Pose] {
        if self.is_player(id) { &[data::Pose::Standing, data::Pose::Crouching, data::Pose::Swimming] } else { &[data::Pose::Standing] }
    }

    pub fn dimensions_for_pose(&self, id: EntityId, pose: data::Pose) -> EntityDimensions {
        if self.is_player(id) { super::avatar::get_default_dimensions(pose) } else { self.entity(id).dimensions }
    }

    pub fn local_bounds_for_pose(&self, id: EntityId, pose: data::Pose) -> AABB {
        let dimensions = self.dimensions_for_pose(id, pose);
        let half_width = (dimensions.width / 2.0) as f64;
        AABB::new(-half_width, 0.0, -half_width, half_width, dimensions.height as f64, half_width)
    }

    pub fn ride_tick(&mut self, id: EntityId) {
        if self.mob(id).is_some_and(super::mob::Mob::is_abstract_skeleton) {
            self.abstract_skeleton_ride_tick(id);
        } else if self.is_drowned(id) {
            self.drowned_ride_tick(id);
        } else {
            self.entity_ride_tick(id);
        }
    }

    pub fn entity_ride_tick(&mut self, id: EntityId) {
        if self.is_player(id) && self.entity(id).is_shift_key_down() && self.is_passenger(id) {
            self.stop_riding(id);
            return;
        }
        let pre = self.entity(id).position();
        self.entity_mut(id).set_delta_movement(Vec3::ZERO);
        self.tick_ridden(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        if let Some(vehicle) = self.vehicle(id) {
            self.position_rider(vehicle, id);
        }
        if self.is_living(id) {
            self.reset_fall_distance(id);
        }
        if self.is_player(id) {
            let now = self.entity(id).position();
            self.check_riding_statistics(id, now.x - pre.x, now.y - pre.y, now.z - pre.z);
        }
    }

    pub fn position_rider(&mut self, id: EntityId, passenger: EntityId) {
        if self.has_passenger(id, passenger) {
            self.position_rider_with(id, passenger, false);
        }
    }

    fn position_rider_with(&mut self, id: EntityId, passenger: EntityId, snap: bool) {
        let position = self.passenger_riding_position(id, passenger);
        let offset = self.vehicle_attachment_point(passenger, id);
        let (x, y, z) = (position.x - offset.x, position.y - offset.y, position.z - offset.z);
        let p = self.entity_mut(passenger);
        if snap {
            let (y_rot, x_rot) = (p.y_rot, p.x_rot);
            p.snap_to(x, y, z, y_rot, x_rot);
        } else {
            p.set_pos(Vec3::new(x, y, z));
        }
        self.entity_moved(passenger);
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_)) => self.abstract_boat_position_rider(id, passenger),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Chicken(_)) => {
                if self.is_living(passenger) {
                    let y_body_rot = self.living(id).y_body_rot;
                    self.living_mut(passenger).y_body_rot = y_body_rot;
                }
            }
            Some(LevelEntity::Mob(mob)) if mob.abstract_horse().is_some() => self.abstract_horse_position_rider(id, passenger),
            _ => {}
        }
    }

    pub fn vehicle_attachment_point(&self, id: EntityId, vehicle: EntityId) -> Vec3 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::Spider(_)) => self.spider_get_vehicle_attachment_point(id, vehicle),
            Some(super::mob::MobKind::CaveSpider(_)) => self.cave_spider_get_vehicle_attachment_point(id, vehicle),
            _ => self.entity_get_vehicle_attachment_point(id, vehicle),
        }
    }

    pub fn entity_get_vehicle_attachment_point(&self, id: EntityId, _vehicle: EntityId) -> Vec3 {
        let e = self.entity(id);
        e.dimensions.attachments.get(EntityAttachment::Vehicle, 0, e.y_rot)
    }

    pub fn passenger_riding_position(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        self.entity(id).position().add_vec(self.passenger_attachment_point(id, passenger))
    }

    fn passenger_attachment_point(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_)) => self.abstract_boat_passenger_attachment_point(id, passenger),
            Some(LevelEntity::Minecart(_)) => self.abstract_minecart_passenger_attachment_point(id, passenger),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Camel(_) | super::mob::MobKind::CamelHusk(_)) => self.camel_passenger_attachment_point(id, passenger),
            Some(LevelEntity::Mob(mob)) if mob.abstract_horse().is_some() => self.abstract_horse_passenger_attachment_point(id, passenger),
            _ => self.default_passenger_attachment_point(id, passenger),
        }
    }

    pub fn default_passenger_attachment_point(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        let vehicle = self.entity(id);
        let passenger_index = vehicle.passengers.iter().position(|&p| p == passenger).map_or(-1, |i| i as i32);
        vehicle.dimensions.attachments.get_clamped(EntityAttachment::Passenger, passenger_index, vehicle.y_rot)
    }

    pub fn fluid_interaction_box(&self, id: EntityId) -> Option<AABB> {
        let box_ = self.entity(id).bounding_box().deflate_all(0.001);
        match self.vehicle(id) {
            Some(vehicle) if self.is_boat(vehicle) => self.abstract_boat_modify_passenger_fluid_interaction_box(vehicle, box_),
            _ => Some(box_),
        }
    }

    pub fn is_in_clouds(&self, id: EntityId) -> bool {
        let position = self.entity(id).position();
        let attributes = self.environment_attributes();
        if attributes.get_value(crate::world::attribute::environment_attributes::CLOUD_COLOR, position)[3] == 0.0 {
            return false;
        }
        let cloud_bottom = attributes.get_value(crate::world::attribute::environment_attributes::CLOUD_HEIGHT, position);
        let e = self.entity(id);
        if e.y() + (e.dimensions.height as f64) < cloud_bottom as f64 {
            return false;
        }
        let cloud_top = cloud_bottom + 4.0;
        e.y() <= cloud_top as f64
    }

    pub fn can_be_collided_with(&self, id: EntityId, _other: Option<EntityId>) -> bool {
        self.is_boat(id) || self.is_shulker(id) && self.is_alive(id) || self.is_happy_ghast(id) && self.happy_ghast_can_be_collided_with(id, _other)
    }

    pub fn can_collide_with(&self, id: EntityId, entity: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_) | LevelEntity::Minecart(_)) => super::vehicle::boat::abstract_boat::can_vehicle_collide(self, id, entity),
            _ if self.is_wind_charge(id) && !self.abstract_wind_charge_can_collide_with(entity) => false,
            _ => self.can_be_collided_with(entity, Some(id)) && !self.is_passenger_of_same_vehicle(id, entity),
        }
    }

    pub fn is_pushable_immutable(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(_) | LevelEntity::Minecart(_)) => true,
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::Bat(_)) => self.bat_is_pushable(id),
            Some(LevelEntity::Mob(mob)) if mob.abstract_horse().is_some() => self.abstract_horse_is_pushable(id),
            _ if self.is_living(id) => self.is_alive(id) && !self.is_spectator(id) && !self.on_climbable_peek(id),
            _ => false,
        }
    }

    pub fn push_entity(&mut self, id: EntityId, other: EntityId) {
        match self.entities.get(&id) {
            Some(LevelEntity::Mob(mob)) if super::monster::shulker::is_shulker(mob) => {}
            Some(LevelEntity::Minecart(_)) => self.abstract_minecart_push(id, other),
            Some(LevelEntity::Boat(_)) => self.abstract_boat_push(id, other),
            Some(LevelEntity::Mob(mob)) if mob.is_cube_mob() => self.abstract_cube_mob_push(id, other),
            _ if self.is_living(id) => self.living_entity_push(id, other),
            _ => self.entity_push_entity(id, other),
        }
    }

    pub fn living_entity_push(&mut self, id: EntityId, other: EntityId) {
        if !self.is_sleeping(id) {
            self.entity_push_entity(id, other);
        }
    }

    pub fn entity_push_entity(&mut self, id: EntityId, other: EntityId) {
        if self.is_passenger_of_same_vehicle(id, other) {
            return;
        }
        let (e, o) = (self.entity(id), self.entity(other));
        if o.no_physics || e.no_physics {
            return;
        }
        let mut xa = o.x() - e.x();
        let mut za = o.z() - e.z();
        let mut dd = mth::abs_max(xa, za);
        if dd < 0.01f32 as f64 {
            return;
        }
        dd = dd.sqrt();
        xa /= dd;
        za /= dd;
        let pow = (1.0 / dd).min(1.0);
        xa *= pow;
        za *= pow;
        xa *= 0.05f32 as f64;
        za *= 0.05f32 as f64;
        if !self.is_vehicle(id) && self.is_pushable(id) {
            self.entity_mut(id).push(-xa, 0.0, -za);
        }
        if !self.is_vehicle(other) && self.is_pushable(other) {
            self.entity_mut(other).push(xa, 0.0, za);
        }
    }

    pub fn push_from_explosion(&mut self, id: EntityId, impulse: Vec3) {
        self.entity_push(id, impulse.x, impulse.y, impulse.z);
    }

    pub fn entity_push(&mut self, id: EntityId, xa: f64, ya: f64, za: f64) {
        if self.is_block_attached_entity(id) {
            self.block_attached_entity_push(id, xa, ya, za);
            return;
        }
        if self.is_wind_charge(id) || self.as_projectile(id).is_some_and(|projectile| projectile.abstract_arrow().is_some() && projectile.is_in_ground()) {
            return;
        }
        if self.is_creaking(id) && !self.creaking_can_move(id) {
            return;
        }
        self.entity_mut(id).push(xa, ya, za);
    }

    pub fn skip_attack_interaction(&mut self, id: EntityId, source: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::ArmorStand(_)) => self.armor_stand_skip_attack_interaction(id, source),
            Some(LevelEntity::Interaction(_)) => self.interaction_skip_attack_interaction(id, source),
            Some(LevelEntity::LeashKnot(_) | LevelEntity::Painting(_) | LevelEntity::ItemFrame(_) | LevelEntity::Cushion(_)) => self.block_attached_entity_skip_attack_interaction(id, source),
            _ => false,
        }
    }

    pub fn get_piston_push_reaction(&self, id: EntityId) -> wafer_data::PushReaction {
        match self.entities.get(&id) {
            Some(LevelEntity::Marker(_) | LevelEntity::AreaEffectCloud(_) | LevelEntity::Interaction(_) | LevelEntity::Display(_) | LevelEntity::OminousItemSpawner(_)) => wafer_data::PushReaction::IgnoreEntity,
            Some(LevelEntity::ArmorStand(stand)) if stand.is_marker() => wafer_data::PushReaction::IgnoreEntity,
            _ => wafer_data::PushReaction::PushPull,
        }
    }

    pub fn is_ignoring_block_triggers(&self, id: EntityId) -> bool {
        if let Some(LevelEntity::Marker(_) | LevelEntity::Interaction(_)) = self.entities.get(&id) {
            return super::marker::Marker::is_ignoring_block_triggers();
        }
        if let Some(LevelEntity::Display(_)) = self.entities.get(&id) {
            return super::display::Display::is_ignoring_block_triggers();
        }
        if let Some(LevelEntity::OminousItemSpawner(_)) = self.entities.get(&id) {
            return super::ominous_item_spawner::OminousItemSpawner::is_ignoring_block_triggers();
        }
        if let Some(LevelEntity::ArmorStand(stand)) = self.entities.get(&id) {
            return stand.is_marker();
        }
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::Bat(_)) => self.bat_is_ignoring_block_triggers(id),
            _ => false,
        }
    }

    pub fn is_pushable(&mut self, id: EntityId) -> bool {
        if self.is_armor_stand(id) {
            return false;
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(super::mob::MobKind::Bat(_))) {
            self.bat_is_pushable(id)
        } else if self.is_abstract_horse(id) {
            self.abstract_horse_is_pushable(id)
        } else if self.is_parrot(id) {
            true
        } else if self.is_creaking(id) {
            self.creaking_is_pushable(id)
        } else if self.is_warden(id) {
            self.warden_is_pushable(id)
        } else if self.is_living(id) {
            self.living_entity_is_pushable(id)
        } else {
            matches!(self.entities.get(&id), Some(LevelEntity::Boat(_) | LevelEntity::Minecart(_)))
        }
    }

    pub fn is_pickable(&self, id: EntityId) -> bool {
        if self.is_player(id) {
            return !self.is_spectator(id) && !self.entity(id).is_removed();
        }
        match self.entities.get(&id) {
            Some(LevelEntity::ArmorStand(stand)) => !stand.entity.is_removed() && !stand.is_marker(),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, super::mob::MobKind::EnderDragon(_)) => false,
            None | Some(LevelEntity::Mob(_) | LevelEntity::Mannequin(_) | LevelEntity::Minecart(_) | LevelEntity::Boat(_) | LevelEntity::PrimedTnt(_) | LevelEntity::FallingBlock(_)) => {
                !self.entity(id).is_removed()
            }
            Some(LevelEntity::EndCrystal(_) | LevelEntity::LeashKnot(_) | LevelEntity::Painting(_) | LevelEntity::ItemFrame(_) | LevelEntity::Cushion(_)) => true,
            Some(LevelEntity::Projectile(projectile)) => {
                matches!(projectile.kind, super::projectile::projectile::ProjectileKind::ShulkerBullet(_))
                    || entity_type_tags::REDIRECTABLE_PROJECTILE.contains(projectile.entity.entity_type as usize) && !(projectile.abstract_arrow().is_some() && projectile.is_in_ground())
            }
            Some(_) => false,
        }
    }

    pub fn pick_radius(&self, _id: EntityId) -> f32 {
        0.0
    }

    pub fn is_animal(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.animal().is_some())
    }

    pub fn set_y_body_rot(&mut self, id: EntityId, y_body_rot: f32) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(super::mob::MobKind::Silverfish(_)) => self.silverfish_set_y_body_rot(id, y_body_rot),
            Some(super::mob::MobKind::Endermite(_)) => self.endermite_set_y_body_rot(id, y_body_rot),
            _ if self.is_living(id) => self.living_entity_set_y_body_rot(id, y_body_rot),
            _ => {}
        }
    }

    pub fn living_entity_set_y_body_rot(&mut self, id: EntityId, y_body_rot: f32) {
        self.living_mut(id).y_body_rot = y_body_rot;
    }

    pub fn clamp_head_rotation_to_body(&mut self, id: EntityId) {
        let Some(mob) = self.mob_mut(id) else { return };
        let max_head_rotation = mob.max_head_y_rot() as f32;
        let head_rotation = mob.entity.y_head_rot;
        let delta = mth::wrap_degrees(mob.living.y_body_rot - head_rotation);
        let target_delta = mth::clamp_f(mth::wrap_degrees(mob.living.y_body_rot - head_rotation), -max_head_rotation, max_head_rotation);
        mob.entity.y_head_rot = head_rotation + delta - target_delta;
    }

    pub fn send_active_effects(&self, entity: EntityId, player: EntityId) {
        let connection = &self.player(player).connection;
        for effect in &self.living(entity).active_effects {
            connection.send_raw(super::living_entity::update_mob_effect(entity, effect, false));
        }
    }

    pub fn get_dimension_changing_delay(&self, id: EntityId) -> i32 {
        if self.is_player(id) {
            return 10;
        }
        if self.is_vehicle_entity(id) {
            return 10;
        }
        if self.is_projectile(id) {
            return 2;
        }
        match self.first_passenger(id) {
            Some(first_passenger) if self.is_player(first_passenger) => self.get_dimension_changing_delay(first_passenger),
            _ => 300,
        }
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::{BlockPos, ChunkPos, Vec3};
    use wafer_data::{blocks, items};

    use crate::server::level::{ServerLevel, testing};
    use crate::world::entity::{ItemEntity, LevelEntity, MoverType};
    use crate::world::item::ItemStack;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn loaded_level() -> ServerLevel {
        let mut level = testing::level();
        for x in -2..=2 {
            for z in -2..=2 {
                level.chunks.chunk(ChunkPos::new(x, z));
            }
        }
        level
    }

    #[test]
    fn dropped_item_lands_on_the_ground_and_rests() {
        let mut level = loaded_level();
        let item = ItemEntity::with_movement(Vec3::new(0.5, -59.0, 0.5), ItemStack::new(items::STONE, 1), Vec3::ZERO);
        let id = item.entity.id;
        level.add_fresh_entity(LevelEntity::Item(item));
        let (mut y, mut v, mut landing_tick) = (-59.0f64, 0.0f64, 0);
        let landed_y = loop {
            landing_tick += 1;
            v -= 0.04;
            if y + v <= -60.0 {
                break y + (-60.0 - y);
            }
            y += v;
            v *= 0.98f32 as f64;
        };
        assert_eq!(landing_tick, 7);
        for tick in 1..=60 {
            level.common_tick(id);
            level.tick_item(id);
            let e = level.entity(id);
            if tick < landing_tick {
                assert!(!e.on_ground, "tick {tick}");
                assert!(e.y() > -60.0, "tick {tick}");
            } else {
                assert!(e.on_ground, "tick {tick}");
                assert_eq!(e.y(), landed_y, "tick {tick}");
                assert_eq!(e.x(), 0.5);
                assert_eq!(e.z(), 0.5);
            }
        }
    }

    #[test]
    fn player_walking_into_a_block_is_stopped() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(2, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.move_entity(id, MoverType::Player, Vec3::new(2.0, 0.0, 0.0));
        let e = level.entity(id);
        let half_width = (0.6f32 / 2.0) as f64;
        assert_eq!(e.x(), 0.5 + (2.0 - (0.5 + half_width)));
        assert_eq!(e.y(), -60.0);
        assert!(e.horizontal_collision);
    }

    #[test]
    fn moving_through_a_block_is_refused() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(2, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(id);
        level.handle_move_player(id, Some(Vec3::new(2.5, -60.0, 0.5)), None, true, false);
        let p = level.player(id);
        assert_eq!(p.entity.position(), Vec3::new(0.5, -60.0, 0.5));
        assert_eq!(p.movement.awaiting_position_from_client, Some(Vec3::new(0.5, -60.0, 0.5)));
    }

    #[test]
    fn player_steps_up_onto_a_slab() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(1, -60, 0), blocks::OAK_SLAB.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.entity_mut(id).on_ground = true;
        level.move_entity(id, MoverType::Player, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(level.entity(id).position(), Vec3::new(1.5, -59.5, 0.5));
    }

    #[test]
    fn a_full_block_is_too_high_to_step_onto() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(1, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.entity_mut(id).on_ground = true;
        level.move_entity(id, MoverType::Player, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(level.entity(id).y(), -60.0);
        assert!(level.entity(id).x() < 0.71);
    }

    #[test]
    fn falling_player_accumulates_fall_distance_and_lands() {
        let mut level = loaded_level();
        let id = testing::player(&mut level, Vec3::new(0.5, -50.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(id);
        let (mut y, mut v) = (-50.0f64, 0.0f64);
        let mut expected = 0.0f64;
        loop {
            level.tick_player(id);
            level.handle_client_tick_end(id);
            v = (v - 0.08) * 0.98f32 as f64;
            let next = (y + v).max(-60.0);
            let landed = next == -60.0;
            level.handle_move_player(id, Some(Vec3::new(0.5, next, 0.5)), None, landed, false);
            assert_eq!(level.entity(id).y(), next);
            expected -= ((next - y) as f32) as f64;
            y = next;
            if landed {
                break;
            }
            assert_eq!(level.entity(id).fall_distance, expected);
            assert!(!level.entity(id).on_ground);
        }
        assert!(level.entity(id).on_ground);
        assert_eq!(level.entity(id).fall_distance, 0.0);
        assert!((expected - 10.0).abs() < 1.0e-4);
        assert_eq!(level.calculate_fall_damage(id, expected, 1.0), 7);
        assert_eq!(level.player(id).health, 13.0);
    }
}
