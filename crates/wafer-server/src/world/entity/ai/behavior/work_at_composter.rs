use wafer_core::BlockPos;
use wafer_data::properties::LEVEL_COMPOSTER;
use wafer_data::{BlockState, Item, blocks, items};

use crate::server::level::ServerLevel;
use crate::server::level::events::COMPOSTER_FILL;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::composter_block;

const COMPOSTABLE_ITEMS: [Item; 2] = [items::WHEAT_SEEDS, items::BEETROOT_SEEDS];

pub fn use_workstation(level: &mut ServerLevel, body: EntityId) {
    let Some(job_site_pos) = level.get_brain(body).get_memory(memory_module_type::JOB_SITE).cloned() else { return };
    let block_state = level.block_state(job_site_pos.pos);
    if block_state.is_block(blocks::COMPOSTER) {
        make_bread(level, body);
        compost_items(level, body, job_site_pos.pos, block_state);
    }
}

fn compost_items(level: &mut ServerLevel, body: EntityId, pos: BlockPos, mut block_state: BlockState) {
    if block_state.get(LEVEL_COMPOSTER) == 8 {
        composter_block::extract_produce(Some(body), level, pos);
        block_state = block_state.set(LEVEL_COMPOSTER, 0);
    }
    let mut total_items_to_use = 20;
    let min_stack_size = 10;
    let mut items_seen_so_far = [0; COMPOSTABLE_ITEMS.len()];
    let container_size = level.abstract_villager(body).inventory.get_container_size();
    let mut temp_state = block_state;
    let mut i = container_size as i32 - 1;
    while i >= 0 && total_items_to_use > 0 {
        let mut item_stack = level.abstract_villager(body).inventory.get_item(i as usize).clone();
        if let Some(item_index) = COMPOSTABLE_ITEMS.iter().position(|&item| item == item_stack.get_item()) {
            let stack_size = item_stack.count;
            let total_item_count = items_seen_so_far[item_index] + stack_size;
            items_seen_so_far[item_index] = total_item_count;
            let items_to_use = (total_item_count - min_stack_size).min(total_items_to_use).min(stack_size);
            if items_to_use > 0 {
                total_items_to_use -= items_to_use;
                for _ in 0..items_to_use {
                    temp_state = composter_block::insert_item(Some(body), temp_state, level, &mut item_stack, pos);
                    level.abstract_villager_mut(body).inventory.items[i as usize] = item_stack.clone();
                    if temp_state.get(LEVEL_COMPOSTER) == 7 {
                        spawn_composter_fill_effects(level, block_state, pos, temp_state);
                        return;
                    }
                }
            }
        }
        i -= 1;
    }
    spawn_composter_fill_effects(level, block_state, pos, temp_state);
}

fn spawn_composter_fill_effects(level: &mut ServerLevel, block_state: BlockState, pos: BlockPos, new_state: BlockState) {
    level.level_event(None, COMPOSTER_FILL, pos, if new_state != block_state { 1 } else { 0 });
}

fn make_bread(level: &mut ServerLevel, body: EntityId) {
    let inventory = &mut level.abstract_villager_mut(body).inventory;
    if inventory.count_item(items::BREAD) <= 36 {
        let how_much_wheat_i_have = inventory.count_item(items::WHEAT);
        let how_much_bread_to_make = 3.min(how_much_wheat_i_have / 3);
        if how_much_bread_to_make != 0 {
            let how_much_wheat_to_use = how_much_bread_to_make * 3;
            inventory.remove_item_type(items::WHEAT, how_much_wheat_to_use);
            let bread_i_cant_carry = inventory.add_item(&ItemStack::new(items::BREAD, how_much_bread_to_make));
            if !bread_i_cant_carry.is_empty() {
                level.spawn_at_location(body, bread_i_cant_carry, 0.5);
            }
        }
    }
}
