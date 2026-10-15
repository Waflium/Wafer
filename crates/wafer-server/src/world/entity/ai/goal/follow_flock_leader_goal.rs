use wafer_core::random::RandomSource;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub const INTERVAL_TICKS: i32 = 200;

pub struct FollowFlockLeaderGoal {
    pub time_to_recalc_path: i32,
    pub next_start_tick: i32,
}

impl FollowFlockLeaderGoal {
    pub fn new(random: &mut impl RandomSource) -> FollowFlockLeaderGoal {
        FollowFlockLeaderGoal { time_to_recalc_path: 0, next_start_tick: next_start_tick(random) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.has_followers(mob) {
            return false;
        }
        if level.is_follower(mob) {
            return true;
        }
        if self.next_start_tick > 0 {
            self.next_start_tick -= 1;
            return false;
        }
        self.next_start_tick = next_start_tick(&mut level.entity_mut(mob).random);
        let area = level.entity(mob).bounding_box().inflate(8.0, 8.0, 8.0);
        let leaders_with_space_or_not_followers: Vec<EntityId> =
            level.get_entities_of_class_same_kind(mob, &area).into_iter().filter(|&fish| level.can_be_followed(fish) || !level.is_follower(fish)).collect();
        let leader = leaders_with_space_or_not_followers.iter().copied().find(|&fish| level.can_be_followed(fish)).unwrap_or(mob);
        let not_followers: Vec<EntityId> = leaders_with_space_or_not_followers.into_iter().filter(|&fish| !level.is_follower(fish)).collect();
        level.add_followers(leader, not_followers);
        level.is_follower(mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.is_follower(mob) && level.in_range_of_leader(mob)
    }

    pub fn start(&mut self) {
        self.time_to_recalc_path = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.stop_following(mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path <= 0 {
            self.time_to_recalc_path = reduced_tick_delay(10);
            level.path_to_leader(mob);
        }
    }
}

fn next_start_tick(random: &mut impl RandomSource) -> i32 {
    reduced_tick_delay(INTERVAL_TICKS + random.next_int_bounded(200) % 20)
}
