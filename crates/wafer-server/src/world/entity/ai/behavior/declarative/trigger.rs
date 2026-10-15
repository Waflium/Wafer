use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub trait Trigger {
    fn trigger(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool;
}

impl<F: FnMut(&mut ServerLevel, EntityId, i64) -> bool> Trigger for F {
    fn trigger(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        self(level, body, timestamp)
    }
}
