use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;

pub const CONTAINER_SIZE: usize = 27;

pub struct MinecartChest;

impl MinecartChest {
    pub fn create() -> AbstractMinecart {
        AbstractMinecart::new("minecraft:chest_minecart", MinecartKind::Chest(MinecartChest))
    }

    pub fn get_default_display_offset() -> i32 {
        8
    }
}

impl ServerLevel {
    pub fn minecart_chest_interact(&mut self, id: EntityId, player: EntityId) -> InteractionResult {
        self.interact_with_container_vehicle(id, player)
    }
}
