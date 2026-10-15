use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::entity_event;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let potential_job_site = i.present(memory_module_type::POTENTIAL_JOB_SITE);
        let job_site = i.registered(memory_module_type::JOB_SITE);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let pos = potential_job_site.get(level, body);
            if !pos.pos.closer_to_center_than(level.entity(body).position(), 2.0) {
                return false;
            }
            potential_job_site.erase(level, body);
            job_site.set(level, body, pos.clone());
            level.broadcast_entity_event(body, entity_event::VILLAGER_HAPPY);
            if level.get_villager_data(body).profession != ProfessionId::NONE {
                return true;
            }
            if pos.dimension == level.dimension
                && let Some(poi_type) = level.chunks.poi_manager.get_type(pos.pos)
                && let Some(profession) = ProfessionId::list_elements().find(|profession| profession.value().held_job_site.test(poi_type))
            {
                let data = level.get_villager_data(body).with_profession(profession);
                level.set_villager_data(body, data);
                level.refresh_brain(body);
            }
            true
        }
    })
}
