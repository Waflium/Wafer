use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};

pub struct MobSensor {
    pub mob_test: fn(&ServerLevel, EntityId, EntityId) -> bool,
    pub ready_test: fn(&ServerLevel, EntityId) -> bool,
    pub to_set: MemoryModuleType<bool>,
    pub memory_time_to_live: i32,
}

impl MobSensor {
    pub fn do_tick(&self, level: &mut ServerLevel, body: EntityId) {
        if !(self.ready_test)(level, body) {
            self.clear_memory(level, body);
        } else {
            self.check_for_mobs_nearby(level, body);
        }
    }

    pub fn requires() -> Vec<MemoryModuleType<Erased>> {
        vec![memory_module_type::NEAREST_LIVING_ENTITIES.erase()]
    }

    pub fn check_for_mobs_nearby(&self, level: &mut ServerLevel, body: EntityId) {
        let Some(living_entities_memory) = level.get_brain(body).get_memory(memory_module_type::NEAREST_LIVING_ENTITIES) else { return };
        let mob_present = living_entities_memory.iter().any(|&entity| (self.mob_test)(level, body, entity));
        if mob_present {
            self.mob_detected(level, body);
        }
    }

    pub fn mob_detected(&self, level: &mut ServerLevel, body: EntityId) {
        level.get_brain_mut(body).set_memory_with_expiry(self.to_set, true, self.memory_time_to_live as i64);
    }

    pub fn clear_memory(&self, level: &mut ServerLevel, body: EntityId) {
        level.get_brain_mut(body).erase_memory(self.to_set);
    }
}
