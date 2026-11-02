use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use crate::world::item::ItemStack;

pub const CONTAINER_SIZE: usize = 27;
pub const EXPANDED_SIZE: usize = CONTAINER_SIZE * 2;

#[derive(Clone)]
pub struct PlayerEnderChestContainer {
    pub items: Vec<ItemStack>,
    active_chest: Option<(BlockPos, u64)>,
}

impl Default for PlayerEnderChestContainer {
    fn default() -> PlayerEnderChestContainer {
        PlayerEnderChestContainer { items: vec![ItemStack::empty(); EXPANDED_SIZE], active_chest: None }
    }
}

impl PlayerEnderChestContainer {
    pub fn set_active_chest(&mut self, active_chest: Option<(BlockPos, u64)>) {
        self.active_chest = active_chest;
    }

    pub fn active_chest(&self) -> Option<(BlockPos, u64)> {
        self.active_chest
    }

    pub fn is_active_chest(&self, pos: BlockPos, id: u64) -> bool {
        self.active_chest == Some((pos, id))
    }

    pub fn from_decoded_slots(&mut self, entries: Vec<(usize, ItemStack)>) {
        self.items = vec![ItemStack::empty(); EXPANDED_SIZE];
        for (slot, stack) in entries {
            if slot < CONTAINER_SIZE {
                self.items[slot] = stack;
            }
        }
    }

    pub fn load_extra(&mut self, extra: Vec<ItemStack>) {
        for (slot, stack) in extra.into_iter().take(CONTAINER_SIZE).enumerate() {
            self.items[CONTAINER_SIZE + slot] = stack;
        }
    }

    pub fn extra(&self) -> &[ItemStack] {
        &self.items[CONTAINER_SIZE..]
    }

    pub fn store_as_slots(&self) -> Nbt {
        Nbt::List(
            self.items
                .iter()
                .enumerate()
                .take(CONTAINER_SIZE)
                .filter(|(_, stack)| !stack.is_empty())
                .filter_map(|(slot, stack)| {
                    let Nbt::Compound(mut fields) = stack.to_nbt() else { return None };
                    fields.insert(0, ("Slot".into(), Nbt::Byte(slot as i8)));
                    Some(Nbt::Compound(fields))
                })
                .collect(),
        )
    }
}
