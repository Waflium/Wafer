use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::memory::memory_value::MemoryValueType;

pub fn create<U: MemoryValueType + 'static>(
    copy_if_true: impl Fn(&mut ServerLevel, EntityId) -> bool + 'static,
    source_memory: MemoryModuleType<U>,
    target_memory: MemoryModuleType<U>,
    duration_of_copy: IntProvider,
) -> OneShot {
    behavior_builder::create(|i| {
        let source = i.present(source_memory);
        let target = i.absent(target_memory);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !copy_if_true(level, body) {
                return false;
            }
            let value = source.get(level, body);
            let duration = duration_of_copy.sample(&mut level.random);
            target.set_with_expiry(level, body, value, duration as i64);
            true
        }
    })
}
