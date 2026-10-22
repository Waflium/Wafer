use wafer_core::Vec3;

use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::containers;
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, RemovalReason};
use crate::world::InteractionResult;

impl ServerLevel {
    pub fn abstract_minecart_container_destroy(&mut self, id: EntityId, source: &DamageSource) {
        let drop_item = self.vehicle_drop_item(id);
        self.destroy(id, drop_item);
        self.chest_vehicle_destroyed(id, source);
    }

    pub fn abstract_minecart_container_remove(&mut self, id: EntityId, reason: RemovalReason) {
        if reason.should_destroy() {
            containers::drop_contents_for_entity(self, id, ContainerRef::ChestVehicle(id));
        }
        self.set_removed(id, reason);
    }

    pub fn abstract_minecart_container_interact(&mut self, id: EntityId, player: EntityId) -> InteractionResult {
        self.interact_with_container_vehicle(id, player)
    }

    pub fn abstract_minecart_container_apply_natural_slowdown(&mut self, id: EntityId, delta_movement: Vec3) -> Vec3 {
        let mut keep = 0.98f32;
        if !self.minecart(id).container.as_ref().expect("a container minecart").has_loot_table() {
            let emptiness = 15 - self.get_redstone_signal_from_container(Some(ContainerRef::ChestVehicle(id)));
            keep += emptiness as f32 * 0.001;
        }
        if self.entity(id).is_in_water() {
            keep *= 0.95;
        }
        delta_movement.multiply(keep as f64, 0.0, keep as f64)
    }
}
