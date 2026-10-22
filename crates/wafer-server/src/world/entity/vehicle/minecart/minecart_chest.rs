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
    pub fn minecart_chest_stop_open(&mut self, id: EntityId, container_user: EntityId) {
        let position = self.entity(id).position();
        self.game_event(crate::world::level::gameevent::game_event::CONTAINER_CLOSE, position, crate::world::level::gameevent::game_event::Context::of(Some(container_user)));
    }

    pub fn minecart_chest_interact(&mut self, id: EntityId, player: EntityId) -> InteractionResult {
        let result = self.interact_with_container_vehicle(id, player);
        if result.consumes_action() {
            self.entity_game_event_by(id, crate::world::level::gameevent::game_event::CONTAINER_OPEN, Some(player));
            crate::world::entity::monster::piglin::piglin_ai::anger_nearby_piglins(self, player, true);
        }
        result
    }
}
