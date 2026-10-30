use wafer_protocol::nbt::Nbt;

use crate::world::item::component::custom;
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    custom::nbt_predicate()
}

use crate::advancements::predicates::nbt_predicate::NbtPredicate;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;

#[derive(Debug)]
pub struct CustomDataPredicate {
    value: NbtPredicate,
}

impl CustomDataPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<CustomDataPredicate> {
        codec::wrapped(Wrap::Xmap, || NbtPredicate::decode(tag)).map(|value| CustomDataPredicate { value })
    }

    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        self.value.matches_components(components)
    }
}
