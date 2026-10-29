use wafer_protocol::nbt::Nbt;

use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::base_spawner::BaseSpawner;

pub struct MinecartSpawner {
    pub spawner: BaseSpawner,
}

fn spawner_mut(minecart: &mut AbstractMinecart) -> &mut BaseSpawner {
    match &mut minecart.kind {
        MinecartKind::Spawner(spawner) => &mut spawner.spawner,
        _ => unreachable!("a spawner minecart"),
    }
}

impl MinecartSpawner {
    pub fn create() -> AbstractMinecart {
        let mut minecart = AbstractMinecart::new("minecraft:spawner_minecart", MinecartKind::Spawner(MinecartSpawner { spawner: BaseSpawner::default() }));
        spawner_mut(&mut minecart).entity_owner = Some(minecart.entity.id);
        minecart
    }

    pub fn read_additional_save_data(minecart: &mut AbstractMinecart, input: &Nbt) {
        spawner_mut(minecart).load(input);
    }

    pub fn add_additional_save_data(minecart: &AbstractMinecart, output: &mut Vec<(String, Nbt)>) {
        if let MinecartKind::Spawner(spawner) = &minecart.kind {
            spawner.spawner.save(output);
        }
    }
}

impl ServerLevel {
    pub fn minecart_spawner_tick(&mut self, id: EntityId) {
        self.abstract_minecart_tick(id);
        self.then_tick(move |level| level.minecart_spawner_tick_end(id));
    }

    fn minecart_spawner_tick_end(&mut self, id: EntityId) {
        let pos = self.entity(id).block_position();
        let mut spawner = std::mem::take(spawner_mut(self.minecart_mut(id)));
        spawner.server_tick(self, pos);
        *spawner_mut(self.minecart_mut(id)) = spawner;
    }
}
