use wafer_data::MobEffect;
use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::{Ints, ints_codec};
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::Wrap;
use crate::world::effect::MobEffectInstance;
use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{self, Pair, def, opt, record};

fn instance_codec() -> Pair {
    record(vec![
        def("amplifier", ints_codec(), types::empty_map()),
        def("duration", ints_codec(), types::empty_map()),
        opt("ambient", types::bool()),
        opt("visible", types::bool()),
    ])
}

pub fn codec() -> Pair {
    (Codec::Map(Box::new(Codec::Element("minecraft:mob_effect")), Box::new(instance_codec().0)), Stream::Nbt)
}

#[derive(Debug)]
pub struct MobEffectInstancePredicate {
    amplifier: Ints,
    duration: Ints,
    ambient: Option<bool>,
    visible: Option<bool>,
}

impl MobEffectInstancePredicate {
    fn decode(tag: &Nbt) -> DataResult<MobEffectInstancePredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let amplifier = record.with_default(tag, "amplifier", Ints::ANY, Ints::decode_partial);
            let duration = record.with_default(tag, "duration", Ints::ANY, Ints::decode_partial);
            let ambient = record.optional(tag, "ambient", codec::bool_value);
            let visible = record.optional(tag, "visible", codec::bool_value);
            record.build(|| Some(MobEffectInstancePredicate { amplifier, duration, ambient, visible }))
        })
        .into_result()
    }

    pub fn matches(&self, instance: Option<&MobEffectInstance>) -> bool {
        let Some(instance) = instance else { return false };
        if !self.amplifier.matches(instance.amplifier) || !self.duration.matches(instance.duration) {
            return false;
        }
        if self.ambient.is_some_and(|ambient| ambient != instance.ambient) {
            return false;
        }
        self.visible.is_none_or(|visible| visible == instance.visible)
    }
}

#[derive(Debug)]
pub struct MobEffectsPredicate {
    effect_map: Vec<(MobEffect, MobEffectInstancePredicate)>,
}

impl MobEffectsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<MobEffectsPredicate> {
        let effect_map = codec::wrapped(Wrap::Xmap, || {
            codec::base_map(
                tag,
                |key| {
                    let id = crate::resources::identifier::read(key)?;
                    MobEffect::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:mob_effect]: {id}"))
                },
                MobEffectInstancePredicate::decode,
            )
        })?;
        Ok(MobEffectsPredicate { effect_map })
    }

    pub fn matches(&self, effects: &[MobEffectInstance]) -> bool {
        self.effect_map.iter().all(|(effect, predicate)| predicate.matches(effects.iter().find(|e| e.effect == *effect)))
    }

    pub fn test(&self, mob_effect: &MobEffectInstance) -> bool {
        self.effect_map.iter().find(|(effect, _)| *effect == mob_effect.effect).is_some_and(|(_, predicate)| predicate.matches(Some(mob_effect)))
    }
}
