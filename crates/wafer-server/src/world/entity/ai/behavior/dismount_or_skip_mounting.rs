use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;

fn is_vehicle_valid(level: &ServerLevel, body: EntityId, vehicle: EntityId, max_walk_dist_to_ride_target: i32) -> bool {
    level.is_alive(vehicle) && level.entity(vehicle).position().closer_than(level.entity(body).position(), max_walk_dist_to_ride_target as f64)
}

pub fn create(max_walk_dist_to_ride_target: i32, dont_ride_if: fn(&ServerLevel, EntityId, EntityId) -> bool) -> OneShot {
    behavior_builder::create(|i| {
        let ride_target = i.registered(memory_module_type::RIDE_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let current_vehicle = level.entity(body).vehicle;
            let target_vehicle = ride_target.try_get(level, body);
            let Some(vehicle) = current_vehicle.or(target_vehicle) else { return false };
            if is_vehicle_valid(level, body, vehicle, max_walk_dist_to_ride_target) && !dont_ride_if(level, body, vehicle) {
                return false;
            }
            level.stop_riding(body);
            ride_target.erase(level, body);
            true
        }
    })
}
