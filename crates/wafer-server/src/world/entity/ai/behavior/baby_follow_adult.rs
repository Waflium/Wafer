use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(follow_range: IntProvider, speed_modifier: f32) -> OneShot {
    create_with(follow_range, move |_, _| speed_modifier, memory_module_type::NEAREST_VISIBLE_ADULT, false)
}

pub fn create_with(
    follow_range: IntProvider,
    speed_modifier: impl Fn(&ServerLevel, EntityId) -> f32 + 'static,
    nearest_visible_type: MemoryModuleType<EntityValue>,
    target_eye: bool,
) -> OneShot {
    let (min_inclusive, max_inclusive) = (follow_range.min_inclusive(), follow_range.max_inclusive());
    behavior_builder::create(|i| {
        let nearest_adult = i.present(nearest_visible_type);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !level.is_baby(body) {
                return false;
            }
            let adult = nearest_adult.get(level, body);
            let Some(adult_entity) = level.entity_ref(adult) else { return false };
            let distance = level.entity(body).distance_to_sqr(adult_entity);
            let closer_than = |dist: f64| distance < dist * dist;
            if closer_than((max_inclusive + 1) as f64) && !closer_than(min_inclusive as f64) {
                let target = WalkTarget::new(PositionTracker::Entity(EntityTracker::with_target_eye_height(level, adult, target_eye, target_eye)), speed_modifier(level, body), min_inclusive - 1);
                look_target.set(level, body, PositionTracker::Entity(EntityTracker::with_target_eye_height(level, adult, true, target_eye)));
                walk_target.set(level, body, target);
                true
            } else {
                false
            }
        }
    })
}
