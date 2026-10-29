use wafer_protocol::nbt::Nbt;

use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::level::block::entity::hopper::Hopper;
use crate::world::level::storage::tag_value_input;

pub const CONTAINER_SIZE: usize = 5;
const DEFAULT_ENABLED: bool = true;

pub struct MinecartHopper {
    enabled: bool,
    consumed_item_this_frame: bool,
}

impl Default for MinecartHopper {
    fn default() -> MinecartHopper {
        MinecartHopper { enabled: DEFAULT_ENABLED, consumed_item_this_frame: false }
    }
}

fn hopper(minecart: &AbstractMinecart) -> &MinecartHopper {
    match &minecart.kind {
        MinecartKind::Hopper(hopper) => hopper,
        _ => unreachable!("a hopper minecart"),
    }
}

fn hopper_mut(minecart: &mut AbstractMinecart) -> &mut MinecartHopper {
    match &mut minecart.kind {
        MinecartKind::Hopper(hopper) => hopper,
        _ => unreachable!("a hopper minecart"),
    }
}

impl MinecartHopper {
    pub fn create() -> AbstractMinecart {
        AbstractMinecart::new("minecraft:hopper_minecart", MinecartKind::Hopper(MinecartHopper::default()))
    }

    pub fn get_default_display_offset() -> i32 {
        1
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn add_additional_save_data(minecart: &AbstractMinecart, output: &mut Vec<(String, Nbt)>) {
        output.push(("Enabled".into(), Nbt::Byte(hopper(minecart).enabled as i8)));
    }

    pub fn read_additional_save_data(minecart: &mut AbstractMinecart, input: &Nbt) {
        hopper_mut(minecart).enabled = tag_value_input::get_i64(input, "Enabled").map_or(DEFAULT_ENABLED, |v| v != 0);
    }
}

impl ServerLevel {
    pub fn minecart_hopper_activate_minecart(&mut self, id: EntityId, state: bool) {
        let hopper = hopper_mut(self.minecart_mut(id));
        let new_enabled = !state;
        if new_enabled != hopper.is_enabled() {
            hopper.set_enabled(new_enabled);
        }
    }

    pub fn minecart_hopper_tick(&mut self, id: EntityId) {
        hopper_mut(self.minecart_mut(id)).consumed_item_this_frame = false;
        self.abstract_minecart_tick(id);
        self.then_tick(move |level| level.minecart_hopper_try_consume_items(id));
    }

    fn minecart_hopper_try_consume_items(&mut self, id: EntityId) {
        let hopper = hopper(self.minecart(id));
        if self.is_alive(id) && hopper.is_enabled() && !hopper.consumed_item_this_frame && self.minecart_hopper_suck_in_items(id) {
            hopper_mut(self.minecart_mut(id)).consumed_item_this_frame = true;
        }
    }

    pub fn minecart_hopper_suck_in_items(&mut self, id: EntityId) -> bool {
        if self.suck_in_items(Hopper::Minecart(id)) {
            return true;
        }
        let area = self.entity(id).bounding_box().inflate(0.25, 0.0, 0.25);
        let items: Vec<EntityId> = self.get_entities(None, &area).into_iter().filter(|&entity| matches!(self.find_entity(entity), Some(crate::world::entity::LevelEntity::Item(_))) && self.is_alive(entity)).collect();
        for entity in items {
            if self.hopper_add_item_entity(ContainerRef::ChestVehicle(id), entity, true) {
                return true;
            }
        }
        false
    }
}
