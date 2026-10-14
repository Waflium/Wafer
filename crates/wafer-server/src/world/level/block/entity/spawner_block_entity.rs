use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::ServerLevel;
use crate::world::level::base_spawner::BaseSpawner;

#[derive(Clone, Default)]
pub struct SpawnerBlockEntity {
    pub spawner: BaseSpawner,
}

impl SpawnerBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.spawner.load(input);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.spawner.save(output);
    }

    pub fn server_tick(level: &mut ServerLevel, pos: BlockPos) {
        let Some(BlockEntityKind::Spawner(entity)) = level.get_block_entity(pos).map(|e| &mut e.kind) else { return };
        let mut spawner = std::mem::take(&mut entity.spawner);
        spawner.server_tick(level, pos);
        if let Some(BlockEntityKind::Spawner(entity)) = level.get_block_entity(pos).map(|e| &mut e.kind) {
            entity.spawner = spawner;
        }
    }

    pub fn trigger_event(&mut self, b0: i32, _b1: i32) -> bool {
        self.spawner.on_event_triggered(b0)
    }
}
