use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::{EntityId, entity_types};

const SPEED_MODIFIER: f32 = 0.3;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let meeting_point = i.present(memory_module_type::MEETING_POINT);
        let nearest_entities = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        let interaction_target = i.absent(memory_module_type::INTERACTION_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let memory = meeting_point.get(level, body);
            let visible_entities = nearest_entities.get(level, body);
            let is_villager = |level: &mut ServerLevel, mob: EntityId| level.entity(mob).entity_type == entity_types::VILLAGER.id;
            if level.random.next_int_bounded(100) == 0
                && level.dimension == memory.dimension
                && memory.pos.closer_to_center_than(level.entity(body).position(), 4.0)
                && visible_entities.contains_matching(level, is_villager)
            {
                let closest = visible_entities.find_closest(level, |level, mob| is_villager(level, mob) && level.entity(mob).position().distance_to_sqr(level.entity(body).position()) <= 32.0);
                if let Some(mob) = closest {
                    interaction_target.set(level, body, mob);
                    let look = PositionTracker::Entity(EntityTracker::new(level, mob, true));
                    look_target.set(level, body, look);
                    let walk = WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, mob, false)), SPEED_MODIFIER, 1);
                    walk_target.set(level, body, walk);
                }
                true
            } else {
                false
            }
        }
    })
}
