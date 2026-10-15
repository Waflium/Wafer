use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::{game_rule, item_tags};

use super::behavior::{Behavior, BehaviorState};
use super::block_pos_tracker::BlockPosTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::npc::villager::villager;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::crop_block;
use crate::world::level::update_flags::UPDATE_ALL;

const HARVEST_DURATION: i32 = 200;
pub const SPEED_MODIFIER: f32 = 0.5;

pub struct HarvestFarmland {
    behavior: BehaviorState,
    above_farmland_pos: Option<BlockPos>,
    next_ok_start_time: i64,
    time_worked_so_far: i32,
    valid_farmland_around_villager: Vec<BlockPos>,
}

impl Default for HarvestFarmland {
    fn default() -> HarvestFarmland {
        HarvestFarmland {
            behavior: BehaviorState::new(vec![
                (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::SECONDARY_JOB_SITE.erase(), MemoryStatus::ValuePresent),
            ]),
            above_farmland_pos: None,
            next_ok_start_time: 0,
            time_worked_so_far: 0,
            valid_farmland_around_villager: Vec::new(),
        }
    }
}

impl HarvestFarmland {
    fn get_valid_farmland(&self, level: &mut ServerLevel) -> Option<BlockPos> {
        if self.valid_farmland_around_villager.is_empty() {
            None
        } else {
            Some(self.valid_farmland_around_villager[level.random.next_int_bounded(self.valid_farmland_around_villager.len() as i32) as usize])
        }
    }

    fn valid_pos(&self, block_pos: BlockPos, level: &mut ServerLevel) -> bool {
        let state = level.block_state(block_pos);
        let block_below = level.block_state(block_pos.below()).block();
        crop_block::as_crop(state.block()).is_some_and(|crop| crop_block::is_max_age(crop, state)) || state.is_air() && block_below.is_instance_of("FarmlandBlock")
    }

    fn set_targets(level: &mut ServerLevel, body: EntityId, pos: BlockPos) {
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::WALK_TARGET, WalkTarget::new(PositionTracker::BlockPos(BlockPosTracker::new(pos)), SPEED_MODIFIER, 1));
        brain.set_memory(memory_module_type::LOOK_TARGET, PositionTracker::BlockPos(BlockPosTracker::new(pos)));
    }
}

impl Behavior for HarvestFarmland {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            return false;
        }
        if level.get_villager_data(body).profession != ProfessionId::FARMER {
            return false;
        }
        self.valid_farmland_around_villager.clear();
        let e = level.entity(body);
        let (bx, by, bz) = (e.x(), e.y(), e.z());
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let mut_pos = BlockPos::containing(bx + x as f64, by + y as f64, bz + z as f64);
                    if self.valid_pos(mut_pos, level) {
                        self.valid_farmland_around_villager.push(mut_pos);
                    }
                }
            }
        }
        self.above_farmland_pos = self.get_valid_farmland(level);
        self.above_farmland_pos.is_some()
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if timestamp > self.next_ok_start_time
            && let Some(pos) = self.above_farmland_pos
        {
            let brain = level.get_brain_mut(body);
            brain.set_memory(memory_module_type::LOOK_TARGET, PositionTracker::BlockPos(BlockPosTracker::new(pos)));
            brain.set_memory(memory_module_type::WALK_TARGET, WalkTarget::new(PositionTracker::BlockPos(BlockPosTracker::new(pos)), SPEED_MODIFIER, 1));
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::LOOK_TARGET);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        self.time_worked_so_far = 0;
        self.next_ok_start_time = timestamp + 40;
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if self.above_farmland_pos.is_some_and(|pos| !pos.closer_to_center_than(level.entity(body).position(), 1.0)) {
            return;
        }
        if let Some(above_farmland_pos) = self.above_farmland_pos
            && timestamp > self.next_ok_start_time
        {
            let block_state = level.block_state(above_farmland_pos);
            let block = block_state.block();
            let block_below = level.block_state(above_farmland_pos.below()).block();
            if crop_block::as_crop(block).is_some_and(|crop| crop_block::is_max_age(crop, block_state)) {
                level.destroy_block(above_farmland_pos, true, Some(body), 512);
            }
            if block_state.is_air() && block_below.is_instance_of("FarmlandBlock") && villager::has_farm_seeds(level.mob(body).expect("a villager")) {
                let size = level.abstract_villager(body).inventory.get_container_size();
                for i in 0..size {
                    let item_stack = level.abstract_villager(body).inventory.get_item(i).clone();
                    let mut ok = false;
                    if !item_stack.is_empty()
                        && item_stack.get_item().is(&item_tags::VILLAGER_PLANTABLE_SEEDS)
                        && let Some(block) = item_stack.get_item().info().block
                    {
                        let place = block.default_state();
                        level.set_block(above_farmland_pos, place, UPDATE_ALL);
                        ok = true;
                    }
                    if ok {
                        let (x, y, z) = (above_farmland_pos.x as f64, above_farmland_pos.y as f64, above_farmland_pos.z as f64);
                        level.play_sound_at(None, x, y, z, sound_event("minecraft:item.crop.plant"), SoundSource::Blocks, 1.0, 1.0);
                        let inventory = &mut level.abstract_villager_mut(body).inventory;
                        inventory.items[i].shrink(1);
                        if inventory.items[i].is_empty() {
                            inventory.set_item(i, ItemStack::empty());
                        }
                        break;
                    }
                }
            }
            if crop_block::as_crop(block).is_some_and(|crop| !crop_block::is_max_age(crop, block_state)) {
                self.valid_farmland_around_villager.retain(|&pos| pos != above_farmland_pos);
                self.above_farmland_pos = self.get_valid_farmland(level);
                if let Some(pos) = self.above_farmland_pos {
                    self.next_ok_start_time = timestamp + 20;
                    HarvestFarmland::set_targets(level, body, pos);
                }
            }
        }
        self.time_worked_so_far += 1;
    }

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        self.time_worked_so_far < HARVEST_DURATION
    }
}
