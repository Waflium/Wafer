use std::cell::RefCell;

use serde_json::Value as Json;
use wafer_protocol::nbt::Nbt;

use crate::util::easing_type::EasingType;
use crate::util::keyframe_track::{KeyframeTrack, KeyframeTrackSampler};
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::attribute::modifier::{AttributeModifier, OperationId};
use crate::world::attribute::Value;

pub mod timelines {
    use crate::util::argb;

    pub fn night_sky_light_color() -> i32 {
        argb::color_from_float(1.0, 0.48, 0.48, 1.0)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AttributeTrack {
    pub modifier: AttributeModifier,
    pub argument_track: KeyframeTrack,
}

impl AttributeTrack {
    pub fn parse(attribute: &EnvironmentAttributeInfo, json: &Json) -> Result<AttributeTrack, String> {
        let modifier = match json.get("modifier") {
            None => AttributeModifier::Override,
            Some(op) => {
                let name = op.as_str().ok_or("modifier must be a string")?;
                OperationId::by_name(name).and_then(|op| attribute.ty.modifier(op)).ok_or_else(|| format!("unknown modifier {name} for {}", attribute.name))?
            }
        };
        let argument_track = KeyframeTrack::parse(json, |v| modifier.parse_argument(attribute, v))?;
        Ok(AttributeTrack { modifier, argument_track })
    }

    pub fn encode(&self, attribute: &EnvironmentAttributeInfo) -> Result<Nbt, String> {
        let mut fields = Vec::new();
        if self.modifier != AttributeModifier::Override {
            fields.push(("modifier".to_string(), Nbt::String(self.modifier.operation_id().name().into())));
        }
        let keyframes = self
            .argument_track
            .keyframes
            .iter()
            .map(|(ticks, value)| Ok(Nbt::Compound(vec![("ticks".into(), Nbt::Int(*ticks)), ("value".into(), self.modifier.encode_argument(attribute, value)?)])))
            .collect::<Result<_, String>>()?;
        fields.push(("keyframes".into(), Nbt::List(keyframes)));
        if self.argument_track.easing_type != EasingType::LINEAR {
            fields.push(("ease".into(), self.argument_track.easing_type.encode()));
        }
        Ok(Nbt::Compound(fields))
    }

    pub fn bake_sampler(&self, attribute: &EnvironmentAttributeInfo, clock: usize, period_ticks: Option<i32>) -> AttributeTrackSampler {
        AttributeTrackSampler {
            clock,
            modifier: self.modifier,
            argument_sampler: self.argument_track.bake_sampler(period_ticks, self.modifier.argument_keyframe_lerp(attribute)),
            cached: RefCell::new(None),
        }
    }
}

#[derive(Debug)]
pub struct AttributeTrackSampler {
    clock: usize,
    modifier: AttributeModifier,
    argument_sampler: KeyframeTrackSampler,
    cached: RefCell<Option<(i32, Value)>>,
}

impl AttributeTrackSampler {
    pub fn apply_time_based(&self, base_value: &Value, cache_tick_id: i32, total_ticks: impl FnOnce(usize) -> i64) -> Value {
        let mut cached = self.cached.borrow_mut();
        if cached.as_ref().is_none_or(|(id, _)| *id != cache_tick_id) {
            *cached = Some((cache_tick_id, self.argument_sampler.sample(total_ticks(self.clock))));
        }
        self.modifier.apply(base_value, &cached.as_ref().unwrap().1)
    }
}
