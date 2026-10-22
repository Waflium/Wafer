use wafer_core::hash_map::java_hash_map_order_with_capacity;
use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{EXTENDED, FACING, PISTON_TYPE, PistonType};
use wafer_data::{Block, BlockState, PushReaction, blocks};

use super::BlockBehaviour;
use super::entity::BlockEntity;
use super::entity::piston_moving_block_entity::PistonMovingBlockEntity;
use super::mirror::Mirror;
use super::piston_structure_resolver::PistonStructureResolver;
use super::rotation::Rotation;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

pub const TRIGGER_EXTEND: i32 = 0;
pub const TRIGGER_CONTRACT: i32 = 1;
pub const TRIGGER_DROP: i32 = 2;

pub struct PistonBaseBlock {
    pub is_sticky: bool,
}

fn block_pos_hash(pos: BlockPos) -> i32 {
    pos.y.wrapping_add(pos.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(pos.x)
}

pub fn new_moving_block_entity(pos: BlockPos, block_state: BlockState, moved_state: BlockState, direction: Direction, extending: bool, is_source_piston: bool) -> BlockEntity {
    let mut entity = BlockEntity::new(wafer_data::block_entity_types::PISTON, pos, block_state).expect("a moving piston");
    entity.kind = super::entity::BlockEntityKind::PistonMoving(PistonMovingBlockEntity::new(moved_state, direction, extending, is_source_piston));
    entity
}

pub fn is_pushable(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, allow_destroyable: bool, connection_direction: Direction) -> bool {
    if pos.y < level.chunks.min_y() || pos.y > level.chunks.max_y() || !level.world_border.is_within_bounds_pos(pos) {
        return false;
    }
    if state.is_air() {
        return true;
    }
    if direction == Direction::Down && pos.y == level.chunks.min_y() {
        return false;
    }
    if direction == Direction::Up && pos.y == level.chunks.max_y() {
        return false;
    }
    if !state.is_block(blocks::PISTON) && !state.is_block(blocks::STICKY_PISTON) {
        if state.destroy_speed() == -1.0 {
            return false;
        }
        match state.push_reaction() {
            PushReaction::Immoveable => return false,
            PushReaction::Popped => return allow_destroyable,
            PushReaction::Push => return direction == connection_direction,
            _ => {}
        }
    } else if state.get(EXTENDED) {
        return false;
    }
    !state.has_block_entity()
}

impl PistonBaseBlock {
    fn base_block(&self) -> Block {
        if self.is_sticky { blocks::STICKY_PISTON } else { blocks::PISTON }
    }

    fn piston_type(&self) -> PistonType {
        if self.is_sticky { PistonType::Sticky } else { PistonType::Default }
    }

    fn check_if_extend(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let direction = state.get(FACING);
        let extend = self.get_neighbor_signal(level, pos, direction);
        if extend && !state.get(EXTENDED) {
            if PistonStructureResolver::new(pos, direction, true).resolve(level) {
                level.block_event(pos, self.base_block(), TRIGGER_EXTEND, direction.ordinal() as i32);
            }
        } else if !extend && state.get(EXTENDED) {
            let pushed_pos = pos.relative_n(direction, 2);
            let pushed_state = level.block_state(pushed_pos);
            let mut event = TRIGGER_CONTRACT;
            let game_time = level.game_time;
            let handling_tick = level.handling_tick;
            if pushed_state.is_block(blocks::MOVING_PISTON)
                && pushed_state.get(FACING) == direction
                && let Some(piston_entity) = level.piston_moving_block_entity(pushed_pos)
                && piston_entity.is_extending()
                && (piston_entity.get_progress(0.0) < 0.5 || game_time == piston_entity.get_last_ticked() || handling_tick)
            {
                event = TRIGGER_DROP;
            }
            level.block_event(pos, self.base_block(), event, direction.ordinal() as i32);
        }
    }

    fn get_neighbor_signal(&self, level: &mut ServerLevel, pos: BlockPos, push_direction: Direction) -> bool {
        for direction in Direction::VALUES {
            if direction != push_direction && level.has_signal(pos.relative(direction), direction) {
                return true;
            }
        }
        if level.has_signal(pos, Direction::Down) {
            return true;
        }
        let above = pos.above();
        for direction in Direction::VALUES {
            if direction != Direction::Down && level.has_signal(above.relative(direction), direction) {
                return true;
            }
        }
        false
    }

    fn move_blocks(&self, level: &mut ServerLevel, piston_pos: BlockPos, direction: Direction, extending: bool) -> bool {
        let arm_pos = piston_pos.relative(direction);
        if !extending && level.block_state(arm_pos).is_block(blocks::PISTON_HEAD) {
            level.set_block(arm_pos, blocks::AIR.default_state(), 276);
        }
        let mut resolver = PistonStructureResolver::new(piston_pos, direction, extending);
        if !resolver.resolve(level) {
            return false;
        }
        let mut delete_after_move: Vec<(BlockPos, BlockState)> = Vec::new();
        let to_push = resolver.get_to_push().to_vec();
        let mut to_push_shapes = Vec::with_capacity(to_push.len());
        for &pos in &to_push {
            let state = level.block_state(pos);
            to_push_shapes.push(state);
            match delete_after_move.iter_mut().find(|(p, _)| *p == pos) {
                Some(entry) => entry.1 = state,
                None => delete_after_move.push((pos, state)),
            }
        }
        let mut delete_after_move_capacity = 16;
        while delete_after_move.len() > delete_after_move_capacity * 3 / 4 {
            delete_after_move_capacity *= 2;
        }
        let to_destroy = resolver.get_to_destroy().to_vec();
        let mut to_update: Vec<BlockState> = Vec::with_capacity(to_push.len() + to_destroy.len());
        let push_direction = if extending { direction } else { direction.opposite() };
        for &pos in to_destroy.iter().rev() {
            let state = level.block_state(pos);
            let block_entity = if state.has_block_entity() { level.get_block_entity(pos).cloned() } else { None };
            level.drop_resources_with_entity(state, pos, block_entity.as_ref(), None, None);
            level.set_block(pos, blocks::AIR.default_state(), 18);
            level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_DESTROY, pos, crate::world::level::gameevent::game_event::Context::of_state(Some(state)));
            to_update.push(state);
        }
        for i in (0..to_push.len()).rev() {
            let block_state = level.block_state(to_push[i]);
            let pos = to_push[i].relative(push_direction);
            delete_after_move.retain(|(p, _)| *p != pos);
            let actual_state = blocks::MOVING_PISTON.default_state().set(FACING, direction);
            level.set_block(pos, actual_state, 324);
            level.set_block_entity(new_moving_block_entity(pos, actual_state, to_push_shapes[i], direction, extending, false));
            to_update.push(block_state);
        }
        if extending {
            let state = blocks::PISTON_HEAD.default_state().set(FACING, direction).set(PISTON_TYPE, self.piston_type());
            let block_state = blocks::MOVING_PISTON.default_state().set(FACING, direction).set(PISTON_TYPE, self.piston_type());
            delete_after_move.retain(|(p, _)| *p != arm_pos);
            level.set_block(arm_pos, block_state, 324);
            level.set_block_entity(new_moving_block_entity(arm_pos, block_state, state, direction, true, true));
        }
        let hashes: Vec<i32> = delete_after_move.iter().map(|(pos, _)| block_pos_hash(*pos)).collect();
        let delete_after_move: Vec<(BlockPos, BlockState)> = java_hash_map_order_with_capacity(&hashes, delete_after_move_capacity).into_iter().map(|i| delete_after_move[i]).collect();
        let air = blocks::AIR.default_state();
        for &(pos, _) in &delete_after_move {
            level.set_block(pos, air, 82);
        }
        for &(pos, old_state) in &delete_after_move {
            super::of(old_state).update_indirect_neighbour_shapes(old_state, level, pos, 2, crate::world::level::update_flags::UPDATE_LIMIT);
            level.update_neighbour_shapes(air, pos, 2, crate::world::level::update_flags::UPDATE_LIMIT);
            super::of(air).update_indirect_neighbour_shapes(air, level, pos, 2, crate::world::level::update_flags::UPDATE_LIMIT);
        }
        let mut update_index = 0;
        for &pos in to_destroy.iter().rev() {
            let state = to_update[update_index];
            update_index += 1;
            super::of(state).affect_neighbors_after_removal(state, level, pos, false);
            super::of(state).update_indirect_neighbour_shapes(state, level, pos, 2, crate::world::level::update_flags::UPDATE_LIMIT);
            level.update_neighbors_at(pos, state.block());
        }
        for &pos in to_push.iter().rev() {
            let block = to_update[update_index].block();
            update_index += 1;
            level.update_neighbors_at(pos, block);
        }
        if extending {
            level.update_neighbors_at(arm_pos, blocks::PISTON_HEAD);
        }
        true
    }
}

impl BlockBehaviour for PistonBaseBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(FACING, ctx.nearest_looking_direction().opposite()).set(EXTENDED, false))
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        self.check_if_extend(level, pos, state);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        self.check_if_extend(level, pos, state);
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) && level.get_block_entity(pos).is_none() {
            self.check_if_extend(level, pos, state);
        }
    }

    fn trigger_event(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        let direction = state.get(FACING);
        let extended_state = state.set(EXTENDED, true);
        let extend = self.get_neighbor_signal(level, pos, direction);
        if extend && (b0 == TRIGGER_CONTRACT || b0 == TRIGGER_DROP) {
            level.set_block(pos, extended_state, 2);
            return false;
        }
        if !extend && b0 == TRIGGER_EXTEND {
            return false;
        }
        if b0 == TRIGGER_EXTEND {
            if !self.move_blocks(level, pos, direction, true) {
                return false;
            }
            level.set_block(pos, extended_state, 67);
            let pitch = level.random.next_float() * 0.25 + 0.6;
            level.play_sound(None, pos, sound_event("minecraft:block.piston.extend"), SoundSource::Blocks, 0.5, pitch);
            level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_ACTIVATE, pos, crate::world::level::gameevent::game_event::Context::of_state(Some(extended_state)));
        } else if b0 == TRIGGER_CONTRACT || b0 == TRIGGER_DROP {
            if level.piston_moving_block_entity(pos.relative(direction)).is_some() {
                level.piston_moving_final_tick(pos.relative(direction));
            }
            let moving_piston_state = blocks::MOVING_PISTON.default_state().set(FACING, direction).set(PISTON_TYPE, self.piston_type());
            level.set_block(pos, moving_piston_state, 276);
            let moved_state = self.base_block().default_state().set(FACING, Direction::from_3d_data_value(b1 & 7));
            level.set_block_entity(new_moving_block_entity(pos, moving_piston_state, moved_state, direction, false, true));
            level.update_neighbors_at(pos, moving_piston_state.block());
            level.update_neighbour_shapes(moving_piston_state, pos, 2, crate::world::level::update_flags::UPDATE_LIMIT);
            if self.is_sticky {
                let two_pos = pos.offset(direction.step_x() * 2, direction.step_y() * 2, direction.step_z() * 2);
                let moving_state = level.block_state(two_pos);
                let mut piston_piece = false;
                if moving_state.is_block(blocks::MOVING_PISTON)
                    && let Some(entity) = level.piston_moving_block_entity(two_pos)
                    && entity.get_direction() == direction
                    && entity.is_extending()
                {
                    level.piston_moving_final_tick(two_pos);
                    piston_piece = true;
                }
                if !piston_piece {
                    if b0 != TRIGGER_CONTRACT
                        || moving_state.is_air()
                        || !is_pushable(moving_state, level, two_pos, direction.opposite(), false, direction)
                        || moving_state.push_reaction() != PushReaction::PushPull && !moving_state.is_block(blocks::PISTON) && !moving_state.is_block(blocks::STICKY_PISTON)
                    {
                        level.remove_block(pos.relative(direction), false);
                    } else {
                        self.move_blocks(level, pos, direction, false);
                    }
                }
            } else {
                level.remove_block(pos.relative(direction), false);
            }
            let pitch = level.random.next_float() * 0.15 + 0.6;
            level.play_sound(None, pos, sound_event("minecraft:block.piston.contract"), SoundSource::Blocks, 0.5, pitch);
            level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_DEACTIVATE, pos, crate::world::level::gameevent::game_event::Context::of_state(Some(moving_piston_state)));
        }
        true
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }
}
