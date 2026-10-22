use wafer_core::BlockPos;
use wafer_data::BlockState;
use wafer_data::properties::TRIAL_SPAWNER_STATE;
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use super::trialspawner::trial_spawner::{self, TrialSpawner};
use crate::server::level::ServerLevel;
use crate::world::level::spawn_data::SpawnData;

#[derive(Clone, Default)]
pub struct TrialSpawnerBlockEntity {
    trial_spawner: TrialSpawner,
}

impl TrialSpawnerBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.trial_spawner.load(input);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.trial_spawner.store(output);
    }

    pub fn get_update_tag(&self, state: BlockState) -> Nbt {
        self.trial_spawner.get_state_data().get_update_tag(state.try_get(TRIAL_SPAWNER_STATE).unwrap_or(wafer_data::properties::TrialSpawnerState::Inactive))
    }

    pub fn get_trial_spawner(&self) -> &TrialSpawner {
        &self.trial_spawner
    }
}

impl ServerLevel {
    fn trial_spawner_with(&mut self, pos: BlockPos, f: impl FnOnce(&mut ServerLevel, &mut TrialSpawner)) {
        let Some(BlockEntityKind::TrialSpawner(entity)) = self.get_block_entity(pos).map(|e| &mut e.kind) else { return };
        let mut trial_spawner = std::mem::take(&mut entity.trial_spawner);
        f(self, &mut trial_spawner);
        if let Some(BlockEntityKind::TrialSpawner(entity)) = self.get_block_entity(pos).map(|e| &mut e.kind) {
            entity.trial_spawner = trial_spawner;
        }
    }

    pub fn trial_spawner_server_tick(&mut self, pos: BlockPos, state: BlockState) {
        let is_ominous = trial_spawner::block_state_is_ominous(state);
        self.trial_spawner_with(pos, |level, trial_spawner| trial_spawner.tick_server(level, pos, is_ominous));
    }

    pub fn trial_spawner_set_entity_data(&mut self, pos: BlockPos, entity_type: &str, entity_data: Option<&Nbt>) {
        let mut spawn_data = SpawnData::new(Vec::new(), None, None);
        spawn_data.get_entity_to_spawn_mut().push(("id".into(), Nbt::String(entity_type.into())));
        if let Some(Nbt::Compound(data)) = entity_data {
            crate::nbt::merge(spawn_data.get_entity_to_spawn_mut(), data);
        }
        self.trial_spawner_with(pos, |level, trial_spawner| trial_spawner.override_entity_to_spawn(level, pos, spawn_data));
        self.block_entity_set_changed(pos);
    }
}
