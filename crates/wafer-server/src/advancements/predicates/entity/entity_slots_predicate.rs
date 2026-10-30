use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::slots_predicate::{self, SlotsPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    slots_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntitySlotsPredicate {
    slots: SlotsPredicate,
}

impl EntitySlotsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntitySlotsPredicate> {
        codec::wrapped(Wrap::Xmap, || SlotsPredicate::decode(tag)).map(|slots| EntitySlotsPredicate { slots })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.slots.matches(|slot| level.get_slot_item(entity, slot))
    }
}
