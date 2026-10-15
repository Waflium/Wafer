use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::entity_type::EntityType;

pub fn create_for_type(entity_type: &'static EntityType, max_dist: f32, interval: IntProvider) -> OneShot {
    create_with(max_dist, interval, move |level, mob| level.entity(mob).entity_type == entity_type.id)
}

fn create_with(max_dist: f32, interval: IntProvider, predicate: impl Fn(&ServerLevel, EntityId) -> bool + 'static) -> OneShot {
    let max_dist_sqr = max_dist * max_dist;
    let mut ticker = Ticker::new(interval);
    behavior_builder::create(|i| {
        let look_target = i.absent(memory_module_type::LOOK_TARGET);
        let nearest_entities = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = nearest_entities
                .get(level, body)
                .find_closest(level, |level, mob| predicate(level, mob) && level.entity(mob).distance_to_sqr(level.entity(body)) <= max_dist_sqr as f64);
            let Some(target) = target else { return false };
            if !ticker.tick_down_and_check(&mut level.random) {
                return false;
            }
            let tracker = EntityTracker::new(level, target, true);
            look_target.set(level, body, PositionTracker::Entity(tracker));
            true
        }
    })
}

pub struct Ticker {
    interval: IntProvider,
    ticks_until_next_start: i32,
}

impl Ticker {
    pub fn new(interval: IntProvider) -> Ticker {
        assert!(interval.min_inclusive() > 1);
        Ticker { interval, ticks_until_next_start: 0 }
    }

    pub fn tick_down_and_check(&mut self, random: &mut impl RandomSource) -> bool {
        if self.ticks_until_next_start == 0 {
            self.ticks_until_next_start = self.interval.sample(random) - 1;
            false
        } else {
            self.ticks_until_next_start -= 1;
            self.ticks_until_next_start == 0
        }
    }
}
