use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::village::poi::poi_types::PoiType;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let job_site = i.present(memory_module_type::JOB_SITE);
        let nearest_entities = i.present(memory_module_type::NEAREST_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let pos = job_site.get(level, body);
            if let Some(poi_type) = level.chunks.poi_manager.get_type(pos.pos) {
                let competitors: Vec<EntityId> = nearest_entities
                    .get(level, body)
                    .into_iter()
                    .filter(|&v| level.is_villager(v) && v != body)
                    .filter(|&v| level.is_alive(v))
                    .filter(|&v| competes_for_same_jobsite(level, &pos, poi_type, v))
                    .collect();
                competitors.into_iter().fold(body, |first, second| select_winner(level, first, second));
            }
            true
        }
    })
}

fn select_winner(level: &mut ServerLevel, first: EntityId, second: EntityId) -> EntityId {
    let (winner, loser) = if level.get_villager_xp(first) > level.get_villager_xp(second) { (first, second) } else { (second, first) };
    level.get_brain_mut(loser).erase_memory(memory_module_type::JOB_SITE);
    winner
}

fn competes_for_same_jobsite(level: &ServerLevel, pos: &GlobalPos, poi_type: PoiType, nearby_villager: EntityId) -> bool {
    let job_site = level.get_brain(nearby_villager).get_memory(memory_module_type::JOB_SITE);
    job_site.is_some_and(|job_site| job_site == pos) && level.get_villager_data(nearby_villager).profession.value().held_job_site.test(poi_type)
}
