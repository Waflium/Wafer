use wafer_protocol::nbt::Nbt;

use super::firework_explosion_predicate::firework_predicate_codec;
use crate::advancements::predicates::collection_predicate;
use crate::advancements::predicates::min_max_bounds::ints_codec;
use crate::world::item::component::types::{self, Pair, def, opt, record};

pub fn codec() -> Pair {
    record(vec![opt("explosions", collection_predicate::codec(firework_predicate_codec)), def("flight_duration", ints_codec(), types::empty_map())])
}

use super::firework_explosion_predicate::FireworkPredicate;
use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec as component_codec;

#[derive(Debug)]
pub struct FireworksPredicate {
    explosions: Option<CollectionPredicate<FireworkPredicate>>,
    flight_duration: Ints,
}

impl FireworksPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FireworksPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let explosions = record.optional(tag, "explosions", |t| CollectionPredicate::decode(t, FireworkPredicate::decode));
            let flight_duration = record.with_default(tag, "flight_duration", Ints::ANY, Ints::decode_partial);
            record.build(|| Some(FireworksPredicate { explosions, flight_duration }))
        })
        .into_result()
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        if let Some(explosions) = &self.explosions {
            let values = value.get("explosions").map_or(&[][..], Nbt::as_list);
            if !explosions.test(values, |p, explosion| p.test(explosion)) {
                return false;
            }
        }
        self.flight_duration.matches(value.get("flight_duration").map_or(0, component_codec::as_i32))
    }
}
