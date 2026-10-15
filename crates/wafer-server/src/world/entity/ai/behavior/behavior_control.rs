use super::behavior::Status;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};

pub trait BehaviorControl: std::any::Any {
    fn get_status(&self) -> Status;

    fn get_required_memories(&self) -> Vec<MemoryModuleType<Erased>>;

    fn try_start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool;

    fn tick_or_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64);

    fn do_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64);

    fn debug_string(&self) -> String;
}
