use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;

pub fn create(radius: i32, speed_modifier: f32, close_enough_dist: i32) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let home = i.registered(memory_module_type::HOME);
        let hiding_place = i.registered(memory_module_type::HIDING_PLACE);
        let path = i.registered(memory_module_type::PATH);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let breed_target = i.registered(memory_module_type::BREED_TARGET);
        let interaction_target = i.registered(memory_module_type::INTERACTION_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let body_pos = level.entity(body).block_position();
            let position = level.entity(body).position();
            let is_home = |poi_type: PoiType| poi_type == PoiType::Home;
            let found = level
                .chunks
                .poi_manager
                .find(&is_home, |_| true, body_pos, close_enough_dist + 1, Occupancy::Any)
                .filter(|p| p.closer_to_center_than(position, close_enough_dist as f64))
                .or_else(|| {
                    let mut random = std::mem::replace(&mut level.entity_mut(body).random, wafer_core::random::LegacyRandomSource::new(0));
                    let pos = level.chunks.poi_manager.get_random(&is_home, |_| true, Occupancy::Any, body_pos, radius, &mut random);
                    level.entity_mut(body).random = random;
                    pos
                })
                .or_else(|| home.try_get(level, body).map(|home| home.pos));
            if let Some(pos) = found {
                path.erase(level, body);
                look_target.erase(level, body);
                breed_target.erase(level, body);
                interaction_target.erase(level, body);
                hiding_place.set(level, body, GlobalPos::of(level.dimension, pos));
                if !pos.closer_to_center_than(position, close_enough_dist as f64) {
                    walk_target.set(level, body, WalkTarget::of_block_pos(pos, speed_modifier, close_enough_dist));
                }
            }
            true
        }
    })
}
