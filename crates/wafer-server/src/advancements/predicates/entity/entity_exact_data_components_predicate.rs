use wafer_protocol::nbt::Nbt;

use crate::core::component::data_component_exact_predicate::DataComponentExactPredicate;
use crate::world::item::component::custom;
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    custom::exact_components()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;
use crate::world::entity::entity_components::EntityComponents;

#[derive(Debug)]
pub struct EntityExactDataComponentsPredicate {
    predicate: DataComponentExactPredicate,
}

impl EntityExactDataComponentsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityExactDataComponentsPredicate> {
        codec::wrapped(Wrap::Xmap, || DataComponentExactPredicate::decode(tag)).map(|predicate| EntityExactDataComponentsPredicate { predicate })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.predicate.test(&EntityComponents { level, id: entity })
    }
}
