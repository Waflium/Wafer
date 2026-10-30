use wafer_protocol::nbt::Nbt;

use crate::core::component::predicates::data_component_predicate::DataComponentPredicate;
use crate::world::item::component::custom;
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    custom::partial_components()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;
use crate::world::entity::entity_components::EntityComponents;

#[derive(Debug)]
pub struct EntityPartialComponentsPredicate {
    predicates: Vec<DataComponentPredicate>,
}

impl EntityPartialComponentsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityPartialComponentsPredicate> {
        codec::wrapped(Wrap::Xmap, || DataComponentPredicate::decode_map(tag)).map(|predicates| EntityPartialComponentsPredicate { predicates })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let components = EntityComponents { level, id: entity };
        self.predicates.iter().all(|predicate| predicate.matches(&components))
    }
}
