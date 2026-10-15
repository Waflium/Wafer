use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::level::BlockGetter;

pub const SCAN_RATE: i32 = 40;

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let dimension_type = level.dimension;
    let center = level.entity(body).block_position();
    let secondary_poi = level.get_villager_data(body).profession.value().secondary_poi;
    let mut job_sites = Vec::new();
    for x in -4..=4 {
        for y in -2..=2 {
            for z in -4..=4 {
                let test_pos = center.offset(x, y, z);
                if secondary_poi.contains(&level.block_state(test_pos).block()) {
                    job_sites.push(GlobalPos::of(dimension_type, test_pos));
                }
            }
        }
    }
    let brain = level.get_brain_mut(body);
    if !job_sites.is_empty() {
        brain.set_memory(memory_module_type::SECONDARY_JOB_SITE, job_sites);
    } else {
        brain.erase_memory(memory_module_type::SECONDARY_JOB_SITE);
    }
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::SECONDARY_JOB_SITE.erase()]
}
