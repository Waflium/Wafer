use wafer_core::BlockPos;

use super::behavior_utils;
use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;

pub fn create(speed_modifier: f32) -> OneShot {
    behavior_builder::create(|i| {
        let potential_job = i.present(memory_module_type::POTENTIAL_JOB_SITE);
        i.absent(memory_module_type::JOB_SITE);
        let nearest_entities = i.present(memory_module_type::NEAREST_LIVING_ENTITIES);
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.is_baby(body) {
                return false;
            }
            if level.get_villager_data(body).profession != ProfessionId::NONE {
                return false;
            }
            let poi_pos = potential_job.get(level, body).pos;
            let Some(poi_type) = level.chunks.poi_manager.get_type(poi_pos) else { return true };
            let candidates = nearest_entities.get(level, body);
            let nearby = candidates.into_iter().find(|&v| level.is_villager(v) && v != body && level.is_alive(v) && nearby_wants_jobsite(level, poi_type, v, poi_pos));
            if let Some(nearby_villager) = nearby {
                walk_target.erase(level, body);
                look_target.erase(level, body);
                potential_job.erase(level, body);
                if level.get_brain(nearby_villager).get_memory(memory_module_type::JOB_SITE).is_none() {
                    behavior_utils::set_walk_and_look_target_memories_to_pos(level, nearby_villager, poi_pos, speed_modifier, 1);
                    let dimension = level.dimension;
                    level.get_brain_mut(nearby_villager).set_memory(memory_module_type::POTENTIAL_JOB_SITE, GlobalPos::of(dimension, poi_pos));
                }
            }
            true
        }
    })
}

fn nearby_wants_jobsite(level: &mut ServerLevel, ty: PoiType, nearby_villager: EntityId, poi_pos: BlockPos) -> bool {
    let brain = level.get_brain(nearby_villager);
    if brain.get_memory(memory_module_type::POTENTIAL_JOB_SITE).is_some() {
        return false;
    }
    let job_site_memory = brain.get_memory(memory_module_type::JOB_SITE).map(|job_site| job_site.pos);
    if !level.get_villager_data(nearby_villager).profession.value().held_job_site.test(ty) {
        return false;
    }
    match job_site_memory {
        None => can_reach_pos(level, nearby_villager, poi_pos, ty),
        Some(job_site) => job_site == poi_pos,
    }
}

fn can_reach_pos(level: &mut ServerLevel, nearby_villager: EntityId, poi_pos: BlockPos, ty: PoiType) -> bool {
    let path = level.get_navigation(nearby_villager, |navigation, level| navigation.create_path(level, nearby_villager, poi_pos, ty.valid_range()));
    path.is_some_and(|path| path.can_reach())
}
