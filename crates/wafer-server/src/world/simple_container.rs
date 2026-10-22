use wafer_data::item_tags;

use crate::world::item::ItemStack;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SimpleContainerKind {
    #[default]
    Plain,
    BeaconPayment,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleContainer {
    pub kind: SimpleContainerKind,
    pub items: Vec<ItemStack>,
}

impl SimpleContainer {
    pub fn new(kind: SimpleContainerKind, size: usize) -> SimpleContainer {
        SimpleContainer { kind, items: vec![ItemStack::empty(); size] }
    }

    pub fn get_max_stack_size(&self) -> i32 {
        match self.kind {
            SimpleContainerKind::Plain => 99,
            SimpleContainerKind::BeaconPayment => 1,
        }
    }

    pub fn can_place_item(&self, _slot: usize, stack: &ItemStack) -> bool {
        match self.kind {
            SimpleContainerKind::Plain => true,
            SimpleContainerKind::BeaconPayment => stack.get_item().is(&item_tags::BEACON_PAYMENT_ITEMS),
        }
    }

    pub fn get_container_size(&self) -> usize {
        self.items.len()
    }

    pub fn get_item(&self, slot: usize) -> &ItemStack {
        &self.items[slot]
    }

    pub fn set_item(&mut self, slot: usize, mut item_stack: ItemStack) {
        let max = self.get_max_stack_size().min(item_stack.max_stack_size());
        item_stack.limit_size(max);
        self.items[slot] = item_stack;
    }

    pub fn remove_item(&mut self, slot: usize, count: i32) -> ItemStack {
        let item = &mut self.items[slot];
        if !item.is_empty() && count > 0 { item.split(count) } else { ItemStack::empty() }
    }

    pub fn remove_item_type(&mut self, item_type: wafer_data::Item, count: i32) -> ItemStack {
        let mut removed = ItemStack::new(item_type, 0);
        for slot in (0..self.items.len()).rev() {
            let current = &mut self.items[slot];
            if current.get_item() == item_type {
                let still_needed = count - removed.count;
                let removed_from_this_slot = current.split(still_needed);
                removed.grow(removed_from_this_slot.count);
                if removed.count == count {
                    break;
                }
            }
        }
        removed
    }

    pub fn add_item(&mut self, item_stack: &ItemStack) -> ItemStack {
        if item_stack.is_empty() {
            return ItemStack::empty();
        }
        let mut remaining_items = item_stack.copy();
        self.move_item_to_occupied_slots_with_same_type(&mut remaining_items);
        if remaining_items.is_empty() {
            return ItemStack::empty();
        }
        self.move_item_to_empty_slots(&mut remaining_items);
        if remaining_items.is_empty() { ItemStack::empty() } else { remaining_items }
    }

    pub fn can_add_item(&self, item_stack: &ItemStack) -> bool {
        self.items.iter().any(|target_stack| target_stack.is_empty() || ItemStack::is_same_item_same_components(target_stack, item_stack) && target_stack.count < target_stack.max_stack_size())
    }

    pub fn has_any_matching(&self, predicate: impl Fn(&ItemStack) -> bool) -> bool {
        self.items.iter().any(|item| !item.is_empty() && predicate(item))
    }

    pub fn count_item(&self, item: wafer_data::Item) -> i32 {
        self.items.iter().filter(|stack| stack.get_item() == item).map(|stack| stack.count).sum()
    }

    pub fn remove_all_items(&mut self) -> Vec<ItemStack> {
        let list = self.items.iter().filter(|item| !item.is_empty()).cloned().collect();
        self.clear_content();
        list
    }

    pub fn clear_content(&mut self) {
        self.items.iter_mut().for_each(|item| *item = ItemStack::empty());
    }

    fn move_item_to_empty_slots(&mut self, source_stack: &mut ItemStack) {
        for slot in 0..self.items.len() {
            if self.items[slot].is_empty() {
                let stack = std::mem::replace(source_stack, ItemStack::empty());
                self.set_item(slot, stack);
                return;
            }
        }
    }

    fn move_item_to_occupied_slots_with_same_type(&mut self, source_stack: &mut ItemStack) {
        for slot in 0..self.items.len() {
            if ItemStack::is_same_item_same_components(&self.items[slot], source_stack) {
                self.move_items_between_stacks(source_stack, slot);
                if source_stack.is_empty() {
                    return;
                }
            }
        }
    }

    fn move_items_between_stacks(&mut self, source_stack: &mut ItemStack, target: usize) {
        let container_max_stack_size = self.get_max_stack_size();
        let target_stack = &mut self.items[target];
        let max_count = container_max_stack_size.min(target_stack.max_stack_size());
        let diff = source_stack.count.min(max_count - target_stack.count);
        if diff > 0 {
            target_stack.grow(diff);
            source_stack.shrink(diff);
        }
    }

    pub fn from_item_list(&mut self, items: &[ItemStack]) {
        self.clear_content();
        for stack in items {
            self.add_item(stack);
        }
    }

    pub fn store_as_item_list(&self) -> Vec<ItemStack> {
        self.items.iter().filter(|item| !item.is_empty()).cloned().collect()
    }
}
