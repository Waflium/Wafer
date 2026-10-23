use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity_predicate::{self, EntityPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    entity_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct TargetedEntityPredicate {
    targeted_entity: Box<EntityPredicate>,
}

impl TargetedEntityPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<TargetedEntityPredicate> {
        Ok(TargetedEntityPredicate { targeted_entity: Box::new(EntityPredicate::decode(tag)?) })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let target = level.mob(entity).and_then(|mob| mob.target);
        self.targeted_entity.matches(level, position, target)
    }
}
