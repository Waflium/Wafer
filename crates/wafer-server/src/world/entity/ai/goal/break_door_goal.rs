use wafer_core::random::RandomSource;
use wafer_data::game_rule;

use super::door_interact_goal::DoorInteractGoal;
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

const DEFAULT_DOOR_BREAK_TIME: i32 = 240;

pub struct BreakDoorGoal {
    pub door_interact_goal: DoorInteractGoal,
    valid_difficulties: fn(Difficulty) -> bool,
    break_time: i32,
    last_break_progress: i32,
    door_break_time: i32,
}

impl BreakDoorGoal {
    pub fn new(valid_difficulties: fn(Difficulty) -> bool) -> BreakDoorGoal {
        BreakDoorGoal { door_interact_goal: DoorInteractGoal::default(), valid_difficulties, break_time: 0, last_break_progress: -1, door_break_time: -1 }
    }

    pub fn with_seconds(seconds: i32, valid_difficulties: fn(Difficulty) -> bool) -> BreakDoorGoal {
        BreakDoorGoal { door_break_time: seconds, ..BreakDoorGoal::new(valid_difficulties) }
    }

    fn get_door_break_time(&self) -> i32 {
        DEFAULT_DOOR_BREAK_TIME.max(self.door_break_time)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.door_interact_goal.can_use(level, mob) {
            return false;
        }
        if !level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            return false;
        }
        self.is_valid_difficulty(level.difficulty) && !self.door_interact_goal.is_open(level)
    }

    pub fn start(&mut self, level: &ServerLevel, mob: EntityId) {
        self.door_interact_goal.start(level, mob);
        self.break_time = 0;
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.break_time <= self.get_door_break_time()
            && !self.door_interact_goal.is_open(level)
            && self.door_interact_goal.door_pos.closer_to_center_than(level.entity(mob).position(), 2.0)
            && self.is_valid_difficulty(level.difficulty)
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.destroy_block_progress(mob, self.door_interact_goal.door_pos, -1);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.door_interact_goal.tick(level, mob);
        let door_pos = self.door_interact_goal.door_pos;
        if level.entity_mut(mob).random.next_int_bounded(20) == 0 {
            level.level_event(None, 1019, door_pos, 0);
            if !level.living(mob).swing_state.is_swinging() {
                let hand = level.used_item_hand(mob);
                level.swing_for_attack(mob, hand);
            }
        }
        self.break_time += 1;
        let progress = (self.break_time as f32 / self.get_door_break_time() as f32 * 10.0) as i32;
        if progress != self.last_break_progress {
            level.destroy_block_progress(mob, door_pos, progress);
            self.last_break_progress = progress;
        }
        if self.break_time == self.get_door_break_time() && self.is_valid_difficulty(level.difficulty) {
            level.remove_block(door_pos, false);
            level.level_event(None, 1021, door_pos, 0);
            let state = level.block_state(door_pos);
            level.level_event(None, 2001, door_pos, state.id() as i32);
        }
    }

    fn is_valid_difficulty(&self, difficulty: Difficulty) -> bool {
        (self.valid_difficulties)(difficulty)
    }
}
