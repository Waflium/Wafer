use std::cell::Cell;

use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, Axis, BlockPos, Direction, Vec3};
use wafer_data::properties::{EXTENDED, FACING, PISTON_TYPE, PistonType, SHORT, WATERLOGGED};
use wafer_data::{BlockState, PushReaction, blocks};
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, MoverType};
use crate::world::level::block::piston_math;
use crate::world::level::chunk::serialization::{state_from_nbt, state_to_nbt};
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

const PUSH_OFFSET: f64 = 0.01;
const DEFAULT_PROGRESS: f32 = 0.0;
const DEFAULT_EXTENDING: bool = false;
const DEFAULT_SOURCE: bool = false;

thread_local! {
    pub static NOCLIP: Cell<Option<Direction>> = const { Cell::new(None) };
}

#[derive(Clone)]
pub struct PistonMovingBlockEntity {
    moved_state: BlockState,
    direction: Direction,
    extending: bool,
    is_source_piston: bool,
    progress: f32,
    progress_o: f32,
    last_ticked: i64,
}

impl Default for PistonMovingBlockEntity {
    fn default() -> PistonMovingBlockEntity {
        PistonMovingBlockEntity {
            moved_state: blocks::AIR.default_state(),
            direction: Direction::Down,
            extending: DEFAULT_EXTENDING,
            is_source_piston: DEFAULT_SOURCE,
            progress: DEFAULT_PROGRESS,
            progress_o: DEFAULT_PROGRESS,
            last_ticked: 0,
        }
    }
}

fn bounds(shape: &VoxelShape) -> AABB {
    AABB::new(shape.min(Axis::X), shape.min(Axis::Y), shape.min(Axis::Z), shape.max(Axis::X), shape.max(Axis::Y), shape.max(Axis::Z))
}

fn get_movement(aabb_to_be_outside_of: &AABB, movement: Direction, aabb: &AABB) -> f64 {
    match movement {
        Direction::East => aabb_to_be_outside_of.max_x - aabb.min_x,
        Direction::West => aabb.max_x - aabb_to_be_outside_of.min_x,
        Direction::Down => aabb.max_y - aabb_to_be_outside_of.min_y,
        Direction::South => aabb_to_be_outside_of.max_z - aabb.min_z,
        Direction::North => aabb.max_z - aabb_to_be_outside_of.min_z,
        Direction::Up => aabb_to_be_outside_of.max_y - aabb.min_y,
    }
}

impl PistonMovingBlockEntity {
    pub fn new(moved_state: BlockState, direction: Direction, extending: bool, is_source_piston: bool) -> PistonMovingBlockEntity {
        PistonMovingBlockEntity { moved_state, direction, extending, is_source_piston, ..PistonMovingBlockEntity::default() }
    }

    pub fn is_extending(&self) -> bool {
        self.extending
    }

    pub fn get_direction(&self) -> Direction {
        self.direction
    }

    pub fn is_source_piston(&self) -> bool {
        self.is_source_piston
    }

    pub fn get_progress(&self, mut a: f32) -> f32 {
        if a > 1.0 {
            a = 1.0;
        }
        wafer_core::mth::lerp_f(a, self.progress_o, self.progress)
    }

    fn get_extended_progress(&self, progress: f32) -> f32 {
        if self.extending { progress - 1.0 } else { 1.0 - progress }
    }

    fn get_collision_related_block_state(&self) -> BlockState {
        if !self.is_extending() && self.is_source_piston() && self.moved_state.block().is_instance_of("PistonBaseBlock") {
            blocks::PISTON_HEAD
                .default_state()
                .set(SHORT, self.progress > 0.25)
                .set(PISTON_TYPE, if self.moved_state.is_block(blocks::STICKY_PISTON) { PistonType::Sticky } else { PistonType::Default })
                .set(FACING, self.moved_state.get(FACING))
        } else {
            self.moved_state
        }
    }

    pub fn get_movement_direction(&self) -> Direction {
        if self.extending { self.direction } else { self.direction.opposite() }
    }

    pub fn get_push_direction(&self) -> Direction {
        if self.extending { self.direction } else { self.direction.opposite() }
    }

    fn move_by_position_and_progress(&self, pos: BlockPos, aabb: AABB) -> AABB {
        let current_position = self.get_extended_progress(self.progress) as f64;
        aabb.move_by(
            pos.x as f64 + current_position * self.direction.step_x() as f64,
            pos.y as f64 + current_position * self.direction.step_y() as f64,
            pos.z as f64 + current_position * self.direction.step_z() as f64,
        )
    }

    fn is_sticky_for_entities(&self) -> bool {
        self.moved_state.is_block(blocks::HONEY_BLOCK)
    }

    pub fn get_moved_state(&self) -> BlockState {
        self.moved_state
    }

    pub fn get_last_ticked(&self) -> i64 {
        self.last_ticked
    }

    pub fn collision_shapes(&self, pos: BlockPos) -> [Option<(&'static VoxelShape, [f64; 3])>; 2] {
        let piston_head_shape = (!self.extending && self.is_source_piston && self.moved_state.block().is_instance_of("PistonBaseBlock"))
            .then(|| (self.moved_state.set(EXTENDED, true).collision_voxel_shape(), [0.0; 3]));
        let no_clip_direction = NOCLIP.with(Cell::get);
        if (self.progress as f64) < 1.0 && no_clip_direction == Some(self.get_movement_direction()) {
            return [piston_head_shape, None];
        }
        let block_state = if self.is_source_piston() {
            blocks::PISTON_HEAD.default_state().set(FACING, self.direction).set(SHORT, self.extending != (1.0 - self.progress < 0.25))
        } else {
            self.moved_state
        };
        let extended_progress = self.get_extended_progress(self.progress);
        let offset = block_state.collision_offset(pos);
        let dx = self.direction.step_x() as f64 * extended_progress as f64;
        let dy = self.direction.step_y() as f64 * extended_progress as f64;
        let dz = self.direction.step_z() as f64 * extended_progress as f64;
        [piston_head_shape, Some((block_state.collision_voxel_shape(), [offset.x + dx, offset.y + dy, offset.z + dz]))]
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.moved_state = input.get("blockState").and_then(state_from_nbt).unwrap_or_else(|| blocks::AIR.default_state());
        self.direction = input.get("facing").and_then(super::int_value).filter(|v| (0..6).contains(v)).map_or(Direction::Down, Direction::from_3d_data_value);
        self.progress = input.get("progress").and_then(super::float_value).unwrap_or(DEFAULT_PROGRESS);
        self.progress_o = self.progress;
        self.extending = input.get("extending").and_then(super::int_value).map_or(DEFAULT_EXTENDING, |v| v != 0);
        self.is_source_piston = input.get("source").and_then(super::int_value).map_or(DEFAULT_SOURCE, |v| v != 0);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("blockState".into(), state_to_nbt(self.moved_state)));
        output.push(("facing".into(), Nbt::Byte(self.direction.ordinal() as i8)));
        output.push(("progress".into(), Nbt::Float(self.progress_o)));
        output.push(("extending".into(), Nbt::Byte(self.extending as i8)));
        output.push(("source".into(), Nbt::Byte(self.is_source_piston as i8)));
    }
}

impl ServerLevel {
    pub fn piston_moving_block_entity(&mut self, pos: BlockPos) -> Option<&mut PistonMovingBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::PistonMoving(piston) => Some(piston),
            _ => None,
        }
    }

    pub fn piston_moving_final_tick(&mut self, pos: BlockPos) {
        let Some(piston) = self.piston_moving_block_entity(pos) else { return };
        if piston.progress_o < 1.0 {
            piston.progress = 1.0;
            piston.progress_o = piston.progress;
            let (is_source_piston, moved_state, push_direction) = (piston.is_source_piston, piston.moved_state, piston.get_push_direction());
            if let Some(mut removed) = self.remove_block_entity(pos) {
                removed.set_removed();
            }
            if self.block_state(pos).is_block(blocks::MOVING_PISTON) {
                let new_state = if is_source_piston { blocks::AIR.default_state() } else { self.update_from_neighbour_shapes(moved_state, pos) };
                self.set_block(pos, new_state, UPDATE_ALL);
                let orientation = crate::world::level::redstone::experimental_redstone_utils::initial_orientation(self, Some(push_direction), None);
                self.neighbor_changed(pos, new_state.block(), orientation);
            }
        }
    }

    pub fn piston_moving_tick(&mut self, pos: BlockPos) {
        let game_time = self.game_time;
        let Some(entity) = self.piston_moving_block_entity(pos) else { return };
        entity.last_ticked = game_time;
        entity.progress_o = entity.progress;
        if entity.progress_o >= 1.0 {
            let (moved_state, push_direction) = (entity.moved_state, entity.get_push_direction());
            if let Some(mut removed) = self.remove_block_entity(pos) {
                removed.set_removed();
            }
            if self.block_state(pos).is_block(blocks::MOVING_PISTON) {
                let mut new_state = self.update_from_neighbour_shapes(moved_state, pos);
                if new_state.is_air() {
                    self.set_block(pos, moved_state, 340);
                    self.update_or_destroy(moved_state, new_state, pos, UPDATE_ALL, crate::world::level::update_flags::UPDATE_LIMIT);
                } else {
                    if new_state.has(WATERLOGGED) && new_state.get(WATERLOGGED) {
                        new_state = new_state.set(WATERLOGGED, false);
                    }
                    self.set_block(pos, new_state, 67);
                    let orientation = crate::world::level::redstone::experimental_redstone_utils::initial_orientation(self, Some(push_direction), None);
                    self.neighbor_changed(pos, new_state.block(), orientation);
                }
            }
        } else {
            let new_progress = entity.progress + 0.5;
            let snapshot = entity.clone();
            self.piston_move_collided_entities(pos, new_progress, &snapshot);
            self.piston_move_stuck_entities(pos, new_progress, &snapshot);
            if let Some(entity) = self.piston_moving_block_entity(pos) {
                entity.progress = new_progress;
                if entity.progress >= 1.0 {
                    entity.progress = 1.0;
                }
            }
        }
    }

    fn piston_move_collided_entities(&mut self, pos: BlockPos, new_progress: f32, piston: &PistonMovingBlockEntity) {
        let movement = piston.get_movement_direction();
        let delta_progress = (new_progress - piston.progress) as f64;
        let state = piston.get_collision_related_block_state();
        let shape = state.collision_voxel_shape();
        if shape.is_empty() {
            return;
        }
        let offset = state.collision_offset(pos);
        let shape_bounds = bounds(shape).move_by(offset.x, offset.y, offset.z);
        let aabb = piston.move_by_position_and_progress(pos, shape_bounds);
        let entities = self.get_entities(None, &piston_math::get_movement_area(&aabb, movement, delta_progress).minmax(&aabb));
        if entities.is_empty() {
            return;
        }
        let shape_aabbs: Vec<AABB> = shape.to_aabbs().into_iter().map(|b| b.move_by(offset.x, offset.y, offset.z)).collect();
        let cause_bounce = piston.moved_state.is_block(blocks::SLIME_BLOCK);
        for entity in entities {
            if self.entity_ref(entity).is_none() || self.get_piston_push_reaction(entity) == PushReaction::IgnoreEntity {
                continue;
            }
            if cause_bounce && self.can_simulate_movement(entity) {
                let delta_movement = self.entity(entity).delta_movement;
                let (mut dx, mut dy, mut dz) = (delta_movement.x, delta_movement.y, delta_movement.z);
                match movement.axis() {
                    Axis::X => dx = movement.step_x() as f64,
                    Axis::Y => dy = movement.step_y() as f64,
                    Axis::Z => dz = movement.step_z() as f64,
                }
                self.entity_mut(entity).set_delta_movement(Vec3::new(dx, dy, dz));
            }
            let mut delta = 0.0f64;
            for shape_aabb in &shape_aabbs {
                let moving_aabb = piston_math::get_movement_area(&piston.move_by_position_and_progress(pos, *shape_aabb), movement, delta_progress);
                let entity_aabb = self.entity(entity).bounding_box();
                if moving_aabb.intersects(&entity_aabb) {
                    delta = delta.max(get_movement(&moving_aabb, movement, &entity_aabb));
                    if delta >= delta_progress {
                        break;
                    }
                }
            }
            if !(delta <= 0.0) {
                delta = delta.min(delta_progress) + PUSH_OFFSET;
                self.move_entity_by_piston(movement, entity, delta, movement);
                if !piston.extending && piston.is_source_piston && self.entity_ref(entity).is_some() {
                    self.fix_entity_within_piston_base(pos, entity, movement, delta_progress);
                }
            }
        }
    }

    fn move_entity_by_piston(&mut self, piston_direction: Direction, entity: EntityId, delta: f64, movement: Direction) {
        NOCLIP.with(|noclip| noclip.set(Some(piston_direction)));
        let previous_pos = self.entity(entity).position();
        self.move_entity(entity, MoverType::Piston, Vec3::new(delta * movement.step_x() as f64, delta * movement.step_y() as f64, delta * movement.step_z() as f64));
        if self.entity_ref(entity).is_some() {
            let position = self.entity(entity).position();
            self.apply_effects_from_blocks_between(entity, previous_pos, position);
            if self.entity_ref(entity).is_some() {
                self.entity_mut(entity).remove_latest_movement_recording();
            }
        }
        NOCLIP.with(|noclip| noclip.set(None));
    }

    fn piston_move_stuck_entities(&mut self, pos: BlockPos, new_progress: f32, piston: &PistonMovingBlockEntity) {
        if !piston.is_sticky_for_entities() {
            return;
        }
        let movement = piston.get_movement_direction();
        if !movement.axis().is_horizontal() {
            return;
        }
        let sticky_top = piston.moved_state.collision_voxel_shape().max(Axis::Y);
        let aabb = piston.move_by_position_and_progress(pos, AABB::new(0.0, sticky_top, 0.0, 1.0, 1.5000010000000001, 1.0));
        let delta_progress = (new_progress - piston.progress) as f64;
        let entities: Vec<EntityId> = self.get_entities(None, &aabb).into_iter().filter(|&entity| self.matches_sticky_criteria(&aabb, entity, pos)).collect();
        for entity in entities {
            if self.entity_ref(entity).is_some() {
                self.move_entity_by_piston(movement, entity, delta_progress, movement);
            }
        }
    }

    fn matches_sticky_criteria(&self, aabb: &AABB, entity: EntityId, pos: BlockPos) -> bool {
        let e = self.entity(entity);
        self.get_piston_push_reaction(entity) == PushReaction::PushPull
            && e.on_ground
            && (e.main_supporting_block_pos == Some(pos) || e.x() >= aabb.min_x && e.x() <= aabb.max_x && e.z() >= aabb.min_z && e.z() <= aabb.max_z)
    }

    fn fix_entity_within_piston_base(&mut self, pos: BlockPos, entity: EntityId, direction: Direction, delta_progress: f64) {
        let entity_aabb = self.entity(entity).bounding_box();
        let block_box = AABB::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0).move_by(pos.x as f64, pos.y as f64, pos.z as f64);
        if entity_aabb.intersects(&block_box) {
            let opposite = direction.opposite();
            let mut delta = get_movement(&block_box, opposite, &entity_aabb) + PUSH_OFFSET;
            let delta_intersected = get_movement(&block_box, opposite, &entity_aabb.intersect(&block_box)) + PUSH_OFFSET;
            if (delta - delta_intersected).abs() < PUSH_OFFSET {
                delta = delta.min(delta_progress) + PUSH_OFFSET;
                self.move_entity_by_piston(direction, entity, delta, opposite);
            }
        }
    }
}
