use wafer_core::random::RandomSource;

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::entity_type::EntityType;

const BREED_RANGE: f64 = 3.0;
const MIN_DURATION: i32 = 60;
const MAX_DURATION: i32 = 110;
const DEFAULT_CLOSE_ENOUGH_DISTANCE: i32 = 2;

pub struct AnimalMakeLove {
    behavior: BehaviorState,
    partner_type: &'static EntityType,
    speed_modifier: f32,
    close_enough_distance: i32,
    spawn_child_at_time: i64,
}

impl AnimalMakeLove {
    pub fn new(partner_type: &'static EntityType) -> AnimalMakeLove {
        AnimalMakeLove::with_speed(partner_type, 1.0, DEFAULT_CLOSE_ENOUGH_DISTANCE)
    }

    pub fn with_speed(partner_type: &'static EntityType, speed_modifier: f32, close_enough_distance: i32) -> AnimalMakeLove {
        AnimalMakeLove {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(), MemoryStatus::ValuePresent),
                    (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
                ],
                MAX_DURATION,
            ),
            partner_type,
            speed_modifier,
            close_enough_distance,
            spawn_child_at_time: 0,
        }
    }

    fn get_breed_target(&self, level: &ServerLevel, body: EntityId) -> EntityId {
        *level.get_brain(body).get_memory(memory_module_type::BREED_TARGET).expect("a breed target")
    }

    fn has_breed_target_of_right_type(&self, level: &ServerLevel, body: EntityId) -> bool {
        let brain = level.get_brain(body);
        brain.has_memory_value(memory_module_type::BREED_TARGET)
            && brain.get_memory(memory_module_type::BREED_TARGET).is_some_and(|&partner| level.entity_ref(partner).is_some_and(|e| e.entity_type == self.partner_type.id))
    }

    fn find_valid_breed_partner(&self, level: &mut ServerLevel, body: EntityId) -> Option<EntityId> {
        let nearest_visible_living_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned().expect("visible entities");
        let partner_type = self.partner_type.id;
        nearest_visible_living_entities.find_closest(level, |level, entity| {
            level.entity(entity).entity_type == partner_type && level.mob(entity).and_then(|mob| mob.animal()).is_some() && level.can_mate(body, entity) && !level.is_panicking(entity)
        })
    }
}

impl Behavior for AnimalMakeLove {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.is_in_love(body) && self.find_valid_breed_partner(level, body).is_some()
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let partner = self.find_valid_breed_partner(level, body).expect("a partner");
        level.get_brain_mut(body).set_memory(memory_module_type::BREED_TARGET, partner);
        level.get_brain_mut(partner).set_memory(memory_module_type::BREED_TARGET, body);
        behavior_utils::lock_gaze_and_walk_to_each_other(level, body, partner, self.speed_modifier, self.close_enough_distance);
        let duration = MIN_DURATION + level.entity_mut(body).random.next_int_bounded(50);
        self.spawn_child_at_time = timestamp + duration as i64;
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        if !self.has_breed_target_of_right_type(level, body) {
            return false;
        }
        let partner = self.get_breed_target(level, body);
        level.is_alive(partner)
            && level.can_mate(body, partner)
            && behavior_utils::entity_is_visible(level, body, partner)
            && timestamp <= self.spawn_child_at_time
            && !level.is_panicking(body)
            && !level.is_panicking(partner)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let partner = self.get_breed_target(level, body);
        behavior_utils::lock_gaze_and_walk_to_each_other(level, body, partner, self.speed_modifier, self.close_enough_distance);
        let distance = level.entity(body).distance_to_sqr(level.entity(partner));
        if distance < BREED_RANGE * BREED_RANGE && timestamp >= self.spawn_child_at_time {
            level.spawn_child_from_breeding(body, partner);
            level.get_brain_mut(body).erase_memory(memory_module_type::BREED_TARGET);
            level.get_brain_mut(partner).erase_memory(memory_module_type::BREED_TARGET);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::BREED_TARGET);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::LOOK_TARGET);
        self.spawn_child_at_time = 0;
    }
}
