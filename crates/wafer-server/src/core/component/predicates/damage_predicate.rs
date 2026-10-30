use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::min_max_bounds::ints_codec;
use crate::world::item::component::types::{self, Pair, def, record};

pub fn codec() -> Pair {
    record(vec![def("durability", ints_codec(), types::empty_map()), def("damage", ints_codec(), types::empty_map())])
}

use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::{self as dc, codec as component_codec};

#[derive(Debug)]
pub struct DamagePredicate {
    durability: Ints,
    damage: Ints,
}

impl DamagePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DamagePredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let durability = record.with_default(tag, "durability", Ints::ANY, Ints::decode_partial);
            let damage = record.with_default(tag, "damage", Ints::ANY, Ints::decode_partial);
            record.build(|| Some(DamagePredicate { durability, damage }))
        })
        .into_result()
    }

    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        let Some(damage) = components.get(dc::DAMAGE) else { return false };
        let damage = component_codec::as_i32(&damage);
        let max_damage = components.get(dc::MAX_DAMAGE).map_or(0, |v| component_codec::as_i32(&v));
        self.durability.matches(max_damage - damage) && self.damage.matches(damage)
    }
}
