use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::MemoryValueType;

pub fn create<U: MemoryValueType + 'static>(pacifying_memory: MemoryModuleType<U>, pacify_duration: i32) -> OneShot {
    behavior_builder::create(|i| {
        let attack_target = i.registered(memory_module_type::ATTACK_TARGET);
        let pacified = i.absent(memory_module_type::PACIFIED);
        let _pacifying = i.present(pacifying_memory);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            pacified.set_with_expiry(level, body, true, pacify_duration as i64);
            attack_target.erase(level, body);
            true
        }
    })
}
