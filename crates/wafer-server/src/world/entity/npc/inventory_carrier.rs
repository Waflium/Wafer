use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::simple_container::SimpleContainer;

pub const TAG_INVENTORY: &str = "Inventory";

pub fn read_inventory_from_tag(inventory: &mut SimpleContainer, input: &Nbt) {
    if let Some(Nbt::List(list)) = input.get(TAG_INVENTORY) {
        let items: Vec<ItemStack> = list.iter().filter_map(|tag| ItemStack::from_nbt(tag).ok()).collect();
        inventory.from_item_list(&items);
    }
}

pub fn write_inventory_to_tag(inventory: &SimpleContainer, output: &mut Vec<(String, Nbt)>) {
    output.push((TAG_INVENTORY.into(), Nbt::List(inventory.store_as_item_list().iter().map(ItemStack::to_nbt).collect())));
}

impl ServerLevel {
    pub fn inventory_carrier_pick_up_item(&mut self, mob: EntityId, item_entity: EntityId, inventory: impl Fn(&mut ServerLevel) -> &mut SimpleContainer) {
        let Some(LevelEntity::Item(item)) = self.entities.get(&item_entity) else { return };
        let item_stack = item.item.clone();
        if self.wants_to_pick_up(mob, &item_stack) {
            let has_space = inventory(self).can_add_item(&item_stack);
            if !has_space {
                return;
            }
            self.on_item_pickup(mob, item_entity);
            let count = item_stack.count;
            let remainder = inventory(self).add_item(&item_stack);
            self.take(mob, item_entity, count - remainder.count);
            let Some(LevelEntity::Item(item)) = self.entities.get_mut(&item_entity) else { return };
            if remainder.is_empty() {
                self.discard(item_entity);
            } else {
                item.item.count = remainder.count;
            }
        }
    }
}
