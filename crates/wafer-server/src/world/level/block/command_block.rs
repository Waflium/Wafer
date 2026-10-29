use wafer_core::{BlockPos, Direction};
use wafer_data::packets::play::clientbound;
use wafer_data::properties::FACING;
use wafer_data::{Block, BlockState, blocks, game_rule};

use super::entity::command_block_entity::Mode;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::Server;
use crate::server::command_origin::CommandOrigin;
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::base_command_block::CommandBlockOwner;
use crate::world::level::redstone::orientation::Orientation;

pub struct CommandBlock {
    pub automatic: bool,
}

impl CommandBlock {
    fn set_powered_and_update(&self, level: &mut ServerLevel, pos: BlockPos, is_powered: bool) {
        let mode = level.command_block_entity_get_mode(pos);
        let Some(command_block) = level.command_block_entity(pos) else { return };
        let was_powered = command_block.is_powered();
        if is_powered != was_powered {
            command_block.set_powered(is_powered);
            if is_powered {
                if command_block.is_automatic() || mode == Mode::Sequence {
                    return;
                }
                level.command_block_entity_mark_condition_met(pos);
                let block = level.block_state(pos).block();
                level.schedule_tick(pos, block, 1);
            }
        }
    }
}

impl BlockBehaviour for CommandBlock {
    fn neighbor_changed(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _orientation: Option<Orientation>, _moved_by_piston: bool) {
        if level.command_block_entity(pos).is_some() {
            let has_neighbor_signal = level.has_neighbor_signal(pos);
            self.set_powered_and_update(level, pos, has_neighbor_signal);
        }
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.command_block_entity(pos).is_some() && level.can_use_game_master_blocks(player) {
            level.open_command_block(player, pos);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::Pass
        }
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.command_block_entity(pos).map_or(0, |entity| entity.command_block.get_success_count())
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, _placer: Option<EntityId>, item: ItemStack) {
        if level.command_block_entity(pos).is_none() {
            return;
        }
        if !item.has(dc::BLOCK_ENTITY_DATA) {
            let send_command_feedback = level.game_rules.get_bool(game_rule::SEND_COMMAND_FEEDBACK);
            if let Some(entity) = level.command_block_entity(pos) {
                entity.command_block.set_track_output(send_command_feedback);
            }
            level.command_block_entity_set_automatic(pos, self.automatic);
        }
        let has_neighbor_signal = level.has_neighbor_signal(pos);
        self.set_powered_and_update(level, pos, has_neighbor_signal);
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(FACING, ctx.nearest_looking_direction().opposite()))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }
}

impl ServerLevel {
    pub fn open_command_block(&mut self, player: EntityId, pos: BlockPos) {
        let Some(entity) = self.get_block_entity(pos) else { return };
        let tag = entity.save_custom_only();
        let ty = entity.ty();
        self.player(player).connection.send(clientbound::BLOCK_ENTITY_DATA, |p| {
            p.position(pos.x, pos.y, pos.z);
            p.varint(ty.0 as i32);
            tag.write(p);
        });
    }
}

impl Server {
    pub fn command_block_tick(&mut self, level: usize, pos: BlockPos) {
        let state = self.levels[level].block_state(pos);
        let mode = self.levels[level].command_block_entity_get_mode(pos);
        let Some(command_block) = self.levels[level].command_block_entity(pos) else { return };
        let command_set = !command_block.command_block.get_command().is_empty();
        let was_condition_met = command_block.was_condition_met();
        if mode == Mode::Auto {
            self.levels[level].command_block_entity_mark_condition_met(pos);
            if was_condition_met {
                self.command_block_execute(level, pos, state, command_set);
            } else if self.levels[level].command_block_entity_is_conditional(pos)
                && let Some(command_block) = self.levels[level].command_block_entity(pos)
            {
                command_block.command_block.set_success_count(0);
            }
            if self.levels[level].command_block_entity(pos).is_some_and(|entity| entity.is_powered() || entity.is_automatic()) {
                self.levels[level].schedule_tick(pos, state.block(), 1);
            }
        } else if mode == Mode::Redstone {
            if was_condition_met {
                self.command_block_execute(level, pos, state, command_set);
            } else if self.levels[level].command_block_entity_is_conditional(pos)
                && let Some(command_block) = self.levels[level].command_block_entity(pos)
            {
                command_block.command_block.set_success_count(0);
            }
        }
        self.levels[level].update_neighbour_for_output_signal(pos, state.block());
    }

    fn command_block_execute(&mut self, level: usize, pos: BlockPos, state: BlockState, command_set: bool) {
        if command_set {
            self.base_command_block_perform_command(CommandBlockOwner::Block(level, pos), CommandOrigin::CommandBlock);
        } else if let Some(command_block) = self.levels[level].command_block_entity(pos) {
            command_block.command_block.set_success_count(0);
        }
        self.execute_chain(level, pos, state.get(FACING));
    }

    fn execute_chain(&mut self, level: usize, block_pos: BlockPos, mut direction: Direction) {
        let mut pos = block_pos;
        let mut max_iterations = self.levels[level].game_rules.get_int(game_rule::MAX_COMMAND_SEQUENCE_LENGTH);
        loop {
            let keep_going = max_iterations > 0;
            max_iterations -= 1;
            if !keep_going {
                break;
            }
            pos = pos.relative(direction);
            let state = self.levels[level].block_state(pos);
            let block = state.block();
            if !state.is_block(blocks::CHAIN_COMMAND_BLOCK)
                || self.levels[level].command_block_entity(pos).is_none()
                || self.levels[level].command_block_entity_get_mode(pos) != Mode::Sequence
            {
                break;
            }
            let command_block = self.levels[level].command_block_entity(pos).expect("a chain command block");
            if command_block.is_powered() || command_block.is_automatic() {
                if self.levels[level].command_block_entity_mark_condition_met(pos) {
                    if !self.base_command_block_perform_command(CommandBlockOwner::Block(level, pos), CommandOrigin::ChainCommandBlock) {
                        break;
                    }
                    self.levels[level].update_neighbour_for_output_signal(pos, block);
                } else if self.levels[level].command_block_entity_is_conditional(pos)
                    && let Some(command_block) = self.levels[level].command_block_entity(pos)
                {
                    command_block.command_block.set_success_count(0);
                }
            }
            direction = state.get(FACING);
        }
        if max_iterations <= 0 {
            let limit = self.levels[level].game_rules.get_int(game_rule::MAX_COMMAND_SEQUENCE_LENGTH).max(0);
            warn!("Command Block chain tried to execute more than {limit} steps!");
        }
    }
}
