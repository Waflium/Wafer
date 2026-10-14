use wafer_core::random::RandomSource;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::goal::goal::reduced_tick_delay;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::Attribute;

const EMPTY_REACH_CACHE: i32 = 0;
const CAN_REACH_CACHE: i32 = 1;
const CANT_REACH_CACHE: i32 = 2;

pub struct TargetGoal {
    pub must_see: bool,
    must_reach: bool,
    reach_cache: i32,
    reach_cache_time: i32,
    unseen_ticks: i32,
    pub target_mob: Option<EntityId>,
    pub unseen_memory_ticks: i32,
}

impl TargetGoal {
    pub fn new(must_see: bool, must_reach: bool) -> TargetGoal {
        TargetGoal { must_see, must_reach, reach_cache: EMPTY_REACH_CACHE, reach_cache_time: 0, unseen_ticks: 0, target_mob: None, unseen_memory_ticks: 60 }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = level.get_target(mob).or(self.target_mob) else { return false };
        if !level.is_living(target) || !level.can_attack(mob, target) {
            return false;
        }
        if let (Some(mob_team), Some(target_team)) = (level.team_of(mob), level.team_of(target))
            && mob_team.name() == target_team.name()
        {
            return false;
        }
        let within = get_follow_distance(level, mob);
        if level.entity(mob).distance_to_sqr(level.entity(target)) > within * within {
            return false;
        }
        if self.must_see {
            if level.sensing_has_line_of_sight(mob, target) {
                self.unseen_ticks = 0;
            } else {
                self.unseen_ticks += 1;
                if self.unseen_ticks > reduced_tick_delay(self.unseen_memory_ticks) {
                    return false;
                }
            }
        }
        level.set_target(mob, Some(target));
        true
    }

    pub fn start(&mut self) {
        self.reach_cache = EMPTY_REACH_CACHE;
        self.reach_cache_time = 0;
        self.unseen_ticks = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, None);
        self.target_mob = None;
    }

    pub fn can_attack(&mut self, level: &mut ServerLevel, mob: EntityId, target: Option<EntityId>, target_conditions: &TargetingConditions) -> bool {
        let Some(target) = target else { return false };
        if !target_conditions.test(level, Some(mob), target) {
            return false;
        }
        let target_pos = level.entity(target).block_position();
        if !level.mob(mob).expect("a mob").is_within_home(target_pos) {
            return false;
        }
        if self.must_reach {
            self.reach_cache_time -= 1;
            if self.reach_cache_time <= 0 {
                self.reach_cache = EMPTY_REACH_CACHE;
            }
            if self.reach_cache == EMPTY_REACH_CACHE {
                self.reach_cache = if self.can_reach(level, mob, target) { CAN_REACH_CACHE } else { CANT_REACH_CACHE };
            }
            if self.reach_cache == CANT_REACH_CACHE {
                return false;
            }
        }
        true
    }

    fn can_reach(&mut self, level: &mut ServerLevel, mob: EntityId, target: EntityId) -> bool {
        self.reach_cache_time = reduced_tick_delay(10 + level.entity_mut(mob).random.next_int_bounded(5));
        let Some(path) = level.get_navigation(mob, |navigation, level| navigation.create_path_to_entity(level, mob, target, 0)) else { return false };
        let Some(last) = path.get_end_node() else { return false };
        let target_pos = level.entity(target).block_position();
        let xx = last.x - target_pos.x;
        let zz = last.z - target_pos.z;
        (xx * xx + zz * zz) as f64 <= 2.25
    }

    pub fn set_unseen_memory_ticks(&mut self, unseen_memory_ticks: i32) {
        self.unseen_memory_ticks = unseen_memory_ticks;
    }
}

pub fn get_follow_distance(level: &ServerLevel, mob: EntityId) -> f64 {
    level.attribute_value(mob, Attribute::FollowRange)
}
