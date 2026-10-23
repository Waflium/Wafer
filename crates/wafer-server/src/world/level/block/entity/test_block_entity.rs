use wafer_core::BlockPos;
use wafer_data::properties::{TEST_BLOCK_MODE, TestBlockMode};
use wafer_data::{BlockState, blocks};
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::storage::tag_value_input;

#[derive(Clone)]
pub struct TestBlockEntity {
    mode: TestBlockMode,
    message: String,
    powered: bool,
    triggered: bool,
}

impl TestBlockEntity {
    pub fn new(state: BlockState) -> TestBlockEntity {
        TestBlockEntity { mode: state.get(TEST_BLOCK_MODE), message: String::new(), powered: false, triggered: false }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("mode".into(), Nbt::String(self.mode.name().into())));
        output.push(("message".into(), Nbt::String(self.message.clone())));
        output.push(("powered".into(), Nbt::Byte(self.powered as i8)));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.mode = input.get("mode").and_then(Nbt::as_str).and_then(TestBlockMode::by_name).unwrap_or(TestBlockMode::Fail);
        self.message = input.get("message").and_then(Nbt::as_str).map(str::to_string).unwrap_or_default();
        self.powered = tag_value_input::get_i64(input, "powered").is_some_and(|powered| powered != 0);
    }

    pub fn is_powered(&self) -> bool {
        self.powered
    }

    pub fn set_powered(&mut self, powered: bool) {
        self.powered = powered;
    }

    pub fn get_mode(&self) -> TestBlockMode {
        self.mode
    }

    pub fn has_triggered(&self) -> bool {
        self.triggered
    }

    pub fn get_message(&self) -> &str {
        &self.message
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    fn log(&self, pos: BlockPos) {
        if !self.message.trim().is_empty() {
            info!("Test {} (at {pos}): {}", self.mode.name(), self.message);
        }
    }
}

impl ServerLevel {
    pub fn test_block_entity(&mut self, pos: BlockPos) -> Option<&mut TestBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::TestBlock(test_block) => Some(test_block),
            _ => None,
        }
    }

    fn test_block_entity_update_block_state(&mut self, pos: BlockPos) {
        let Some(mode) = self.test_block_entity(pos).map(|entity| entity.mode) else { return };
        let block_state = self.block_state(pos);
        if block_state.is_block(blocks::TEST_BLOCK) {
            self.set_block(pos, block_state.set(TEST_BLOCK_MODE, mode), 2);
        }
    }

    pub fn test_block_entity_set_mode(&mut self, pos: BlockPos, mode: TestBlockMode) {
        let Some(entity) = self.test_block_entity(pos) else { return };
        entity.mode = mode;
        self.test_block_entity_update_block_state(pos);
    }

    pub fn test_block_entity_reset(&mut self, pos: BlockPos) {
        let Some(entity) = self.test_block_entity(pos) else { return };
        entity.triggered = false;
        if entity.mode == TestBlockMode::Start {
            entity.set_powered(false);
            let block = self.block_state(pos).block();
            self.update_neighbors_at(pos, block);
        }
    }

    pub fn test_block_entity_trigger(&mut self, pos: BlockPos) {
        let Some(entity) = self.test_block_entity(pos) else { return };
        if entity.mode == TestBlockMode::Start {
            entity.set_powered(true);
            let block = self.block_state(pos).block();
            self.update_neighbors_at(pos, block);
            self.test_block_entity(pos).unwrap().log(pos);
        } else {
            if entity.mode == TestBlockMode::Log {
                entity.log(pos);
            }
            entity.triggered = true;
        }
    }
}
