use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity_predicate::{self, EntityPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    entity_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct TargetedEntityPredicate {
    targeted_entity: Box<EntityPredicate>,
}

impl TargetedEntityPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<TargetedEntityPredicate> {
        codec::wrapped(Wrap::Xmap, || EntityPredicate::decode(tag)).map(|targeted_entity| TargetedEntityPredicate { targeted_entity: Box::new(targeted_entity) })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let target = level.mob(entity).and_then(|mob| mob.target);
        self.targeted_entity.matches(level, position, target)
    }
}
