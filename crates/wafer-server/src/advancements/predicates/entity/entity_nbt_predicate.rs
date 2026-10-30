use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::nbt_predicate::NbtPredicate;
use crate::world::item::component::custom;
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    custom::nbt_predicate()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityNbtPredicate {
    nbt: NbtPredicate,
}

impl EntityNbtPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityNbtPredicate> {
        codec::wrapped(Wrap::Xmap, || NbtPredicate::decode(tag)).map(|nbt| EntityNbtPredicate { nbt })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.nbt.matches(Some(&level.entity_tag_to_compare(entity)))
    }
}
