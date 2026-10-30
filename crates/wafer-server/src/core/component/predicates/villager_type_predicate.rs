use wafer_protocol::nbt::Nbt;

use crate::world::item::component::types::{self, Pair};

pub fn codec() -> Pair {
    types::holder_set("minecraft:villager_type")
}

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::item::component::codec::registry_id;

#[derive(Debug)]
pub struct VillagerTypePredicate {
    villager_types: HolderSet,
}

impl VillagerTypePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<VillagerTypePredicate> {
        codec::wrapped(Wrap::Xmap, || HolderSet::decode("minecraft:villager_type", tag)).map(|villager_types| VillagerTypePredicate { villager_types })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        value.as_str().and_then(|id| registry_id("minecraft:villager_type", id).ok()).is_some_and(|id| self.villager_types.contains(id))
    }
}
