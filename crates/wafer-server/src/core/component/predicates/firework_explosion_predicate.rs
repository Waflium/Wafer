use wafer_protocol::nbt::Nbt;

use crate::world::item::component::types::{self, Pair, opt, record};

pub fn firework_predicate_codec() -> Pair {
    record(vec![
        opt("shape", types::enumeration(&["small_ball", "large_ball", "star", "creeper", "burst"])),
        opt("has_twinkle", types::bool()),
        opt("has_trail", types::bool()),
    ])
}

pub fn codec() -> Pair {
    firework_predicate_codec()
}

use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::Wrap;
use crate::world::item::component::codec as component_codec;

const SHAPES: &[(&str, &str)] = &[("small_ball", "small_ball"), ("large_ball", "large_ball"), ("star", "star"), ("creeper", "creeper"), ("burst", "burst")];

#[derive(Debug)]
pub struct FireworkPredicate {
    shape: Option<&'static str>,
    twinkle: Option<bool>,
    trail: Option<bool>,
}

impl FireworkPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let shape = record.optional(tag, "shape", |t| codec::string_enum(t, SHAPES));
            let twinkle = record.optional(tag, "has_twinkle", codec::bool_value);
            let trail = record.optional(tag, "has_trail", codec::bool_value);
            record.build(|| Some(FireworkPredicate { shape, twinkle, trail }))
        })
        .into_result()
    }

    pub fn test(&self, firework_explosion: &Nbt) -> bool {
        let flag = |key| firework_explosion.get(key).is_some_and(|v| component_codec::as_i64(v) != 0);
        if self.shape.is_some_and(|shape| firework_explosion.get("shape").and_then(Nbt::as_str) != Some(shape)) {
            return false;
        }
        if self.twinkle.is_some_and(|twinkle| twinkle != flag("has_twinkle")) {
            return false;
        }
        self.trail.is_none_or(|trail| trail == flag("has_trail"))
    }
}

#[derive(Debug)]
pub struct FireworkExplosionPredicate {
    predicate: FireworkPredicate,
}

impl FireworkExplosionPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkExplosionPredicate> {
        codec::wrapped(Wrap::Xmap, || FireworkPredicate::decode(tag)).map(|predicate| FireworkExplosionPredicate { predicate })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        self.predicate.test(value)
    }
}
