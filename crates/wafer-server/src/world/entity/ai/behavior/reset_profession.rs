use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        i.absent(memory_module_type::JOB_SITE);
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let body_data = level.get_villager_data(body);
            let can_be_fired = body_data.profession != ProfessionId::NONE && body_data.profession != ProfessionId::NITWIT;
            if can_be_fired && level.get_villager_xp(body) == 0 && body_data.level <= 1 {
                level.set_villager_data(body, body_data.with_profession(ProfessionId::NONE));
                level.refresh_brain(body);
                true
            } else {
                false
            }
        }
    })
}
