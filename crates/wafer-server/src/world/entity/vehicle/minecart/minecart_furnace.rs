use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::item_tags;
use wafer_protocol::nbt::Nbt;

use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::data::DataValue;
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const DATA_ID_FUEL: usize = 13;
const FUEL_TICKS_PER_ITEM: i32 = 3600;
const MAX_FUEL_TICKS: i32 = 32000;
const DEFAULT_FUEL: i16 = 0;
const DEFAULT_PUSH: Vec3 = Vec3::ZERO;

pub struct MinecartFurnace {
    fuel: i32,
    pub push: Vec3,
}

impl Default for MinecartFurnace {
    fn default() -> MinecartFurnace {
        MinecartFurnace { fuel: DEFAULT_FUEL as i32, push: DEFAULT_PUSH }
    }
}

fn furnace(minecart: &AbstractMinecart) -> &MinecartFurnace {
    match &minecart.kind {
        MinecartKind::Furnace(furnace) => furnace,
        _ => unreachable!("a furnace minecart"),
    }
}

fn furnace_mut(minecart: &mut AbstractMinecart) -> &mut MinecartFurnace {
    match &mut minecart.kind {
        MinecartKind::Furnace(furnace) => furnace,
        _ => unreachable!("a furnace minecart"),
    }
}

impl MinecartFurnace {
    pub fn create() -> AbstractMinecart {
        AbstractMinecart::new("minecraft:furnace_minecart", MinecartKind::Furnace(MinecartFurnace::default()))
    }

    fn has_fuel(minecart: &AbstractMinecart) -> bool {
        matches!(minecart.entity.data.get(DATA_ID_FUEL), DataValue::Boolean(true))
    }

    fn set_has_fuel(minecart: &mut AbstractMinecart, fuel: bool) {
        minecart.entity.data.set(DATA_ID_FUEL, DataValue::Boolean(fuel));
    }

    pub fn add_additional_save_data(minecart: &AbstractMinecart, output: &mut Vec<(String, Nbt)>) {
        let furnace = furnace(minecart);
        output.push(("PushX".into(), Nbt::Double(furnace.push.x)));
        output.push(("PushZ".into(), Nbt::Double(furnace.push.z)));
        output.push(("Fuel".into(), Nbt::Short(furnace.fuel as i16)));
    }

    pub fn read_additional_save_data(minecart: &mut AbstractMinecart, input: &Nbt) {
        let x_push = tag_value_input::get_f64(input, "PushX").unwrap_or(DEFAULT_PUSH.x);
        let z_push = tag_value_input::get_f64(input, "PushZ").unwrap_or(DEFAULT_PUSH.z);
        let furnace = furnace_mut(minecart);
        furnace.push = Vec3::new(x_push, 0.0, z_push);
        furnace.fuel = tag_value_input::get_i64(input, "Fuel").map_or(DEFAULT_FUEL, |v| v as i16) as i32;
    }

    fn calculate_new_push_along(&self, delta_movement: Vec3) -> Vec3 {
        if self.push.horizontal_distance_sqr() > 1.0E-4 && delta_movement.horizontal_distance_sqr() > 0.001 {
            self.push.projected_on(delta_movement).normalize().scale(self.push.length())
        } else {
            self.push
        }
    }
}

impl ServerLevel {
    pub fn minecart_furnace_tick(&mut self, id: EntityId) {
        self.abstract_minecart_tick(id);
        self.then_tick(move |level| level.minecart_furnace_tick_end(id));
    }

    fn minecart_furnace_tick_end(&mut self, id: EntityId) {
        let minecart = self.minecart_mut(id);
        let furnace = furnace_mut(minecart);
        if furnace.fuel > 0 {
            furnace.fuel -= 1;
        }
        if furnace.fuel <= 0 {
            furnace.push = Vec3::ZERO;
        }
        let has_fuel = furnace.fuel > 0;
        MinecartFurnace::set_has_fuel(minecart, has_fuel);
        if MinecartFurnace::has_fuel(minecart) {
            minecart.entity.random.next_int_bounded(4);
        }
    }

    pub fn minecart_furnace_max_speed(&self, id: EntityId) -> f64 {
        if self.entity(id).is_in_water() { self.minecart_behavior_max_speed(id) * 0.75 } else { self.minecart_behavior_max_speed(id) * 0.5 }
    }

    pub fn minecart_furnace_apply_natural_slowdown(&mut self, id: EntityId, delta_movement: Vec3) -> Vec3 {
        let in_water = self.entity(id).is_in_water();
        let furnace = furnace_mut(self.minecart_mut(id));
        let new_delta_movement = if furnace.push.length_sqr() > 1.0E-7 {
            furnace.push = furnace.calculate_new_push_along(delta_movement);
            let new_delta_movement = delta_movement.multiply(0.8, 0.0, 0.8).add_vec(furnace.push);
            if in_water { new_delta_movement.scale(0.1) } else { new_delta_movement }
        } else {
            delta_movement.multiply(0.98, 0.0, 0.98)
        };
        self.abstract_minecart_apply_natural_slowdown(id, new_delta_movement)
    }

    pub fn minecart_furnace_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let player_position = self.entity(player).position();
        if self.minecart_furnace_add_fuel(id, player_position, &item_stack) {
            item_stack.consume(1, self, Some(player));
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
        }
        InteractionResult::SUCCESS
    }

    pub fn minecart_furnace_add_fuel(&mut self, id: EntityId, interacting_pos: Vec3, item_stack: &ItemStack) -> bool {
        let position = self.entity(id).position();
        let furnace = furnace_mut(self.minecart_mut(id));
        if item_tags::FURNACE_MINECART_FUEL.contains(item_stack.get_item().0 as usize) && furnace.fuel + FUEL_TICKS_PER_ITEM <= MAX_FUEL_TICKS {
            furnace.fuel += FUEL_TICKS_PER_ITEM;
            if furnace.fuel > 0 {
                furnace.push = position.subtract(interacting_pos).horizontal();
            }
            true
        } else {
            false
        }
    }
}
