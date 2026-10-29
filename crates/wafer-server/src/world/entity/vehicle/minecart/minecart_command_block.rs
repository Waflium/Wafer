use wafer_protocol::nbt::Nbt;

use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::Server;
use crate::server::command_origin::CommandOrigin;
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::entity::data::DataValue;
use crate::world::level::base_command_block::{BaseCommandBlock, CommandBlockOwner};

pub const DATA_ID_COMMAND_NAME: usize = 13;
pub const DATA_ID_LAST_OUTPUT: usize = 14;
const ACTIVATION_DELAY: i32 = 4;

#[derive(Default)]
pub struct MinecartCommandBlock {
    pub command_block: BaseCommandBlock,
    last_activated: i32,
}

fn command_block_minecart_mut(minecart: &mut AbstractMinecart) -> &mut MinecartCommandBlock {
    match &mut minecart.kind {
        MinecartKind::CommandBlock(command_block) => command_block,
        _ => unreachable!("a command block minecart"),
    }
}

impl MinecartCommandBlock {
    pub fn create() -> AbstractMinecart {
        AbstractMinecart::new("minecraft:command_block_minecart", MinecartKind::CommandBlock(MinecartCommandBlock::default()))
    }

    pub fn read_additional_save_data(minecart: &mut AbstractMinecart, input: &Nbt) {
        let command_block = &mut command_block_minecart_mut(minecart).command_block;
        command_block.load(input);
        let (command, last_output) = (command_block.get_command().to_string(), command_block.get_last_output());
        minecart.entity.data.set(DATA_ID_COMMAND_NAME, DataValue::String(command));
        minecart.entity.data.set(DATA_ID_LAST_OUTPUT, DataValue::Component(last_output));
    }

    pub fn add_additional_save_data(minecart: &AbstractMinecart, output: &mut Vec<(String, Nbt)>) {
        if let MinecartKind::CommandBlock(command_block) = &minecart.kind {
            command_block.command_block.save(output);
        }
    }
}

impl ServerLevel {
    pub fn minecart_command_block(&mut self, id: EntityId) -> Option<&mut BaseCommandBlock> {
        match self.find_entity_mut(id) {
            Some(crate::world::entity::LevelEntity::Minecart(minecart)) => match &mut minecart.kind {
                MinecartKind::CommandBlock(command_block) => Some(&mut command_block.command_block),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn minecart_command_block_on_updated(&mut self, id: EntityId) {
        let Some(command_block) = self.minecart_command_block(id) else { return };
        let (command, last_output) = (command_block.get_command().to_string(), command_block.get_last_output());
        let e = self.entity_mut(id);
        e.data.set(DATA_ID_COMMAND_NAME, DataValue::String(command));
        e.data.set(DATA_ID_LAST_OUTPUT, DataValue::Component(last_output));
    }

    pub fn minecart_command_block_activate_minecart(&mut self, id: EntityId, state: bool) {
        let tick_count = self.entity(id).tick_count;
        let minecart = command_block_minecart_mut(self.minecart_mut(id));
        if state && tick_count - minecart.last_activated >= ACTIVATION_DELAY {
            self.pending_minecart_command = Some(id);
            self.suspend_tick(move |level| level.minecart_command_block_activated(id));
        }
    }

    fn minecart_command_block_activated(&mut self, id: EntityId) {
        let tick_count = self.entity(id).tick_count;
        command_block_minecart_mut(self.minecart_mut(id)).last_activated = tick_count;
    }

    pub fn minecart_command_block_interact(&mut self, player: EntityId) -> InteractionResult {
        if !self.can_use_game_master_blocks(player) { InteractionResult::Pass } else { InteractionResult::SUCCESS }
    }
}

impl Server {
    pub fn run_pending_minecart_command(&mut self, level: usize) {
        if let Some(entity) = self.levels[level].pending_minecart_command.take()
            && self.levels[level].minecart_command_block(entity).is_some()
        {
            self.base_command_block_perform_command(CommandBlockOwner::Minecart(level, entity), CommandOrigin::MinecartCommandBlock);
        }
    }
}
