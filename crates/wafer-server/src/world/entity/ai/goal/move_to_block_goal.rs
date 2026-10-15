use wafer_core::BlockPos;
use wafer_core::random::RandomSource;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

const GIVE_UP_TICKS: i32 = 1200;
const STAY_TICKS: i32 = 1200;
const INTERVAL_TICKS: i32 = 200;

pub type IsValidTarget<'a> = &'a dyn Fn(&ServerLevel, BlockPos) -> bool;

pub struct MoveToBlockGoal {
    pub speed_modifier: f64,
    pub next_start_tick: i32,
    pub try_ticks: i32,
    max_stay_ticks: i32,
    pub block_pos: BlockPos,
    reached_target: bool,
    search_range: i32,
    vertical_search_range: i32,
    pub vertical_search_start: i32,
}

impl MoveToBlockGoal {
    pub fn new(speed_modifier: f64, search_range: i32, vertical_search_range: i32) -> MoveToBlockGoal {
        MoveToBlockGoal {
            speed_modifier,
            next_start_tick: 0,
            try_ticks: 0,
            max_stay_ticks: 0,
            block_pos: BlockPos::ZERO,
            reached_target: false,
            search_range,
            vertical_search_range,
            vertical_search_start: 0,
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId, is_valid_target: IsValidTarget) -> bool {
        self.can_use_with(level, mob, next_start_tick, is_valid_target)
    }

    pub fn can_use_with(&mut self, level: &mut ServerLevel, mob: EntityId, next_start_tick_of: impl FnOnce(&mut ServerLevel, EntityId) -> i32, is_valid_target: IsValidTarget) -> bool {
        if self.next_start_tick > 0 {
            self.next_start_tick -= 1;
            false
        } else {
            self.next_start_tick = next_start_tick_of(level, mob);
            self.find_nearest_block(level, mob, is_valid_target)
        }
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, is_valid_target: IsValidTarget) -> bool {
        self.try_ticks >= -self.max_stay_ticks && self.try_ticks <= GIVE_UP_TICKS && is_valid_target(level, self.block_pos)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_mob_to_block(level, mob);
        self.try_ticks = 0;
        let random = &mut level.entity_mut(mob).random;
        let bound = random.next_int_bounded(GIVE_UP_TICKS) + STAY_TICKS;
        self.max_stay_ticks = random.next_int_bounded(bound) + STAY_TICKS;
    }

    fn move_mob_to_block(&self, level: &mut ServerLevel, mob: EntityId) {
        let (pos, speed_modifier) = (self.block_pos, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, pos.x as f64 + 0.5, (pos.y + 1) as f64, pos.z as f64 + 0.5, speed_modifier));
    }

    pub fn get_move_to_target(&self) -> BlockPos {
        self.block_pos.above()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId, accepted_distance: f64) {
        self.tick_with(level, mob, accepted_distance, MoveToBlockGoal::should_recalculate_path);
    }

    pub fn tick_with(&mut self, level: &mut ServerLevel, mob: EntityId, accepted_distance: f64, should_recalculate_path: fn(&MoveToBlockGoal) -> bool) {
        let move_to_target = self.get_move_to_target();
        if !move_to_target.closer_to_center_than(level.entity(mob).position(), accepted_distance) {
            self.reached_target = false;
            self.try_ticks += 1;
            if should_recalculate_path(self) {
                let speed_modifier = self.speed_modifier;
                level.get_navigation(mob, |navigation, level| {
                    navigation.move_to(level, mob, move_to_target.x as f64 + 0.5, move_to_target.y as f64, move_to_target.z as f64 + 0.5, speed_modifier)
                });
            }
        } else {
            self.reached_target = true;
            self.try_ticks -= 1;
        }
    }

    pub fn should_recalculate_path(&self) -> bool {
        self.try_ticks % 40 == 0
    }

    pub fn is_reached_target(&self) -> bool {
        self.reached_target
    }

    pub fn find_nearest_block(&mut self, level: &ServerLevel, mob: EntityId, is_valid_target: IsValidTarget) -> bool {
        let horizontal_search = self.search_range;
        let vertical_search = self.vertical_search_range;
        let m = level.mob(mob).expect("a mob");
        let mob_pos = m.entity.block_position();
        let mut y = self.vertical_search_start;
        while y <= vertical_search {
            for r in 0..horizontal_search {
                let mut x = 0;
                while x <= r {
                    let mut z = if x < r && x > -r { r } else { 0 };
                    while z <= r {
                        let pos = mob_pos.offset(x, y - 1, z);
                        if m.is_within_home(pos) && is_valid_target(level, pos) {
                            self.block_pos = pos;
                            return true;
                        }
                        z = if z > 0 { -z } else { 1 - z };
                    }
                    x = if x > 0 { -x } else { 1 - x };
                }
            }
            y = if y > 0 { -y } else { 1 - y };
        }
        false
    }
}

pub fn next_start_tick(level: &mut ServerLevel, mob: EntityId) -> i32 {
    reduced_tick_delay(INTERVAL_TICKS + level.entity_mut(mob).random.next_int_bounded(INTERVAL_TICKS))
}
