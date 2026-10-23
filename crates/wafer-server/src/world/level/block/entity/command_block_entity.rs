use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{CONDITIONAL, FACING};
use wafer_data::{BlockState, blocks};
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::BlockGetter;
use crate::world::level::base_command_block::BaseCommandBlock;
use crate::world::level::storage::tag_value_input;

const DEFAULT_POWERED: bool = false;
const DEFAULT_CONDITION_MET: bool = false;
const DEFAULT_AUTOMATIC: bool = false;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Sequence,
    Auto,
    Redstone,
}

#[derive(Clone)]
pub struct CommandBlockEntity {
    powered: bool,
    auto: bool,
    condition_met: bool,
    pub command_block: BaseCommandBlock,
    automatic_set_by_load: bool,
}

impl CommandBlockEntity {
    pub fn new(automatic: bool) -> CommandBlockEntity {
        CommandBlockEntity { powered: DEFAULT_POWERED, auto: automatic, condition_met: DEFAULT_CONDITION_MET, command_block: BaseCommandBlock::default(), automatic_set_by_load: false }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.command_block.save(output);
        output.push(("powered".into(), Nbt::Byte(self.powered as i8)));
        output.push(("conditionMet".into(), Nbt::Byte(self.condition_met as i8)));
        output.push(("auto".into(), Nbt::Byte(self.auto as i8)));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        let boolean = |key: &str, default: bool| tag_value_input::get_i64(input, key).map_or(default, |v| v != 0);
        self.command_block.load(input);
        self.powered = boolean("powered", DEFAULT_POWERED);
        self.condition_met = boolean("conditionMet", DEFAULT_CONDITION_MET);
        let previous_auto = self.auto;
        self.auto = boolean("auto", DEFAULT_AUTOMATIC);
        self.automatic_set_by_load = !previous_auto && self.auto && !self.powered;
    }

    pub fn is_powered(&self) -> bool {
        self.powered
    }

    pub fn set_powered(&mut self, powered: bool) {
        self.powered = powered;
    }

    pub fn is_automatic(&self) -> bool {
        self.auto
    }

    pub fn was_condition_met(&self) -> bool {
        self.condition_met
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.command_block.set_custom_name(get(dc::CUSTOM_NAME).cloned());
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        components.retain(|(t, _)| *t != dc::CUSTOM_NAME);
        components.push((dc::CUSTOM_NAME, self.command_block.get_custom_name().cloned()));
    }
}

pub fn get_mode(state: BlockState) -> Mode {
    if state.is_block(blocks::COMMAND_BLOCK) {
        Mode::Redstone
    } else if state.is_block(blocks::REPEATING_COMMAND_BLOCK) {
        Mode::Auto
    } else if state.is_block(blocks::CHAIN_COMMAND_BLOCK) {
        Mode::Sequence
    } else {
        Mode::Redstone
    }
}

impl ServerLevel {
    pub fn command_block_entity(&mut self, pos: BlockPos) -> Option<&mut CommandBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::CommandBlock(command_block) => Some(command_block),
            _ => None,
        }
    }

    pub fn command_block_entity_on_updated(&mut self, pos: BlockPos) {
        let state = self.block_state(pos);
        self.send_block_updated(pos, state, state);
    }

    pub fn command_block_facing(&mut self, pos: BlockPos) -> Direction {
        self.chunks.block_entity_ref(pos).map_or(Direction::North, |entity| entity.block_state().get(FACING))
    }

    pub fn command_block_entity_get_mode(&mut self, pos: BlockPos) -> Mode {
        get_mode(self.chunks.block_entity_ref(pos).map_or_else(|| blocks::AIR.default_state(), |entity| entity.block_state()))
    }

    pub fn command_block_entity_set_automatic(&mut self, pos: BlockPos, auto: bool) {
        let mode = self.command_block_entity_get_mode(pos);
        let Some(entity) = self.command_block_entity(pos) else { return };
        let previous_auto = entity.auto;
        entity.auto = auto;
        if !previous_auto && auto && !entity.powered && mode != Mode::Sequence {
            self.command_block_entity_schedule_tick(pos);
        }
    }

    pub fn block_entity_loaded(&mut self, pos: BlockPos) {
        self.structure_block_entity_update_block_state(pos);
        let mode = self.command_block_entity_get_mode(pos);
        let Some(entity) = self.command_block_entity(pos) else { return };
        if std::mem::take(&mut entity.automatic_set_by_load) && mode != Mode::Sequence {
            self.command_block_entity_schedule_tick(pos);
        }
    }

    pub fn command_block_entity_on_mode_switch(&mut self, pos: BlockPos) {
        let new_mode = self.command_block_entity_get_mode(pos);
        let Some(entity) = self.command_block_entity(pos) else { return };
        if new_mode == Mode::Auto && (entity.powered || entity.auto) {
            self.command_block_entity_schedule_tick(pos);
        }
    }

    fn command_block_entity_schedule_tick(&mut self, pos: BlockPos) {
        let Some(block) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state().block()) else { return };
        if block.is_instance_of("CommandBlock") {
            self.command_block_entity_mark_condition_met(pos);
            self.schedule_tick(pos, block, 1);
        }
    }

    pub fn command_block_entity_mark_condition_met(&mut self, pos: BlockPos) -> bool {
        let conditional = self.command_block_entity_is_conditional(pos);
        let mut condition_met = true;
        if conditional {
            let relative = pos.relative(self.block_state(pos).get(FACING).opposite());
            condition_met = if self.block_state(relative).block().is_instance_of("CommandBlock") {
                self.command_block_entity(relative).is_some_and(|entity| entity.command_block.get_success_count() > 0)
            } else {
                false
            };
        }
        if let Some(entity) = self.command_block_entity(pos) {
            entity.condition_met = condition_met;
        }
        condition_met
    }

    pub fn command_block_entity_is_conditional(&mut self, pos: BlockPos) -> bool {
        let block_state = self.block_state(pos);
        if block_state.block().is_instance_of("CommandBlock") { block_state.get(CONDITIONAL) } else { false }
    }
}
