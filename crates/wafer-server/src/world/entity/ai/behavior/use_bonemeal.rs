use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items};

use super::behavior::{Behavior, BehaviorState};
use super::block_pos_tracker::BlockPosTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_PLANT_GROWTH;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::item::{ItemStack, bone_meal_item};
use crate::world::level::BlockGetter;
use crate::world::level::block::crop_block;

const BONEMEALING_DURATION: i32 = 80;

pub struct UseBonemeal {
    behavior: BehaviorState,
    next_work_cycle_time: i64,
    last_bonemealing_session: i64,
    time_worked_so_far: i32,
    crop_pos: Option<BlockPos>,
}

impl Default for UseBonemeal {
    fn default() -> UseBonemeal {
        UseBonemeal {
            behavior: BehaviorState::new(vec![(memory_module_type::LOOK_TARGET.erase(), MemoryStatus::ValueAbsent), (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent)]),
            next_work_cycle_time: 0,
            last_bonemealing_session: 0,
            time_worked_so_far: 0,
            crop_pos: None,
        }
    }
}

impl UseBonemeal {
    fn pick_next_target(&self, level: &mut ServerLevel, body: EntityId) -> Option<BlockPos> {
        let mut result = None;
        let mut count = 0;
        let body_pos = level.entity(body).block_position();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let mut_pos = body_pos.offset(x, y, z);
                    if self.valid_pos(mut_pos, level) {
                        count += 1;
                        if level.random.next_int_bounded(count) == 0 {
                            result = Some(mut_pos);
                        }
                    }
                }
            }
        }
        result
    }

    fn valid_pos(&self, block_pos: BlockPos, level: &mut ServerLevel) -> bool {
        let state = level.block_state(block_pos);
        crop_block::as_crop(state.block()).is_some_and(|crop| !crop_block::is_max_age(crop, state))
    }

    fn set_current_crop_as_target(&self, level: &mut ServerLevel, body: EntityId) {
        if let Some(pos) = self.crop_pos {
            let crop_pos_wrapper = PositionTracker::BlockPos(BlockPosTracker::new(pos));
            let brain = level.get_brain_mut(body);
            brain.set_memory(memory_module_type::LOOK_TARGET, crop_pos_wrapper.clone());
            brain.set_memory(memory_module_type::WALK_TARGET, WalkTarget::new(crop_pos_wrapper, 0.5, 1));
        }
    }
}

impl Behavior for UseBonemeal {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let tick_count = level.entity(body).tick_count as i64;
        if tick_count % 10 == 0 && (self.last_bonemealing_session == 0 || self.last_bonemealing_session + 160 <= tick_count) {
            if level.abstract_villager(body).inventory.count_item(items::BONE_MEAL) <= 0 {
                return false;
            }
            self.crop_pos = self.pick_next_target(level, body);
            self.crop_pos.is_some()
        } else {
            false
        }
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        self.time_worked_so_far < BONEMEALING_DURATION && self.crop_pos.is_some()
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.set_current_crop_as_target(level, body);
        level.living_set_item_slot(body, EquipmentSlot::Mainhand, ItemStack::new(items::BONE_MEAL, 1));
        self.next_work_cycle_time = timestamp;
        self.time_worked_so_far = 0;
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.living_set_item_slot(body, EquipmentSlot::Mainhand, ItemStack::empty());
        self.last_bonemealing_session = level.entity(body).tick_count as i64;
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let target_pos = self.crop_pos.expect("a crop");
        if timestamp >= self.next_work_cycle_time && target_pos.closer_to_center_than(level.entity(body).position(), 1.0) {
            let size = level.abstract_villager(body).inventory.get_container_size();
            let bonemeal_slot = (0..size).find(|&i| level.abstract_villager(body).inventory.get_item(i).is(items::BONE_MEAL));
            if let Some(slot) = bonemeal_slot {
                let mut bonemeal_stack = level.abstract_villager(body).inventory.get_item(slot).clone();
                let grew = !bonemeal_stack.is_empty() && bone_meal_item::grow_crop(&mut bonemeal_stack, level, target_pos);
                level.abstract_villager_mut(body).inventory.items[slot] = bonemeal_stack;
                if grew {
                    level.level_event(None, PARTICLES_AND_SOUND_PLANT_GROWTH, target_pos, 15);
                    self.crop_pos = self.pick_next_target(level, body);
                    self.set_current_crop_as_target(level, body);
                    self.next_work_cycle_time = timestamp + 40;
                }
            }
            self.time_worked_so_far += 1;
        }
    }
}
