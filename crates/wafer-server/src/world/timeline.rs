use std::cell::RefCell;
use std::sync::LazyLock;

use serde_json::Value as Json;
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use crate::util::codec::{self, DataResult, DecodeResult};
use crate::util::easing_type::EasingType;
use crate::util::hash_ops::{self, HashCode};
use crate::util::keyframe_track::{KeyframeTrack, KeyframeTrackSampler};
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::attribute::environment_attribute_map::{OPERATION_NAMES, argument_codec};
use crate::world::attribute::environment_attributes::ATTRIBUTES;
use crate::world::attribute::modifier::{AttributeModifier, OperationId};
use crate::world::attribute::Value;
use crate::world::item::component::codec::{Codec, Custom, Field, Stream, Wrap, decode_map_codec};
use crate::world::item::component::types::{self, Pair, def, req};

pub mod timelines {
    use crate::util::argb;

    pub fn night_sky_light_color() -> i32 {
        argb::color_from_float(1.0, 0.48, 0.48, 1.0)
    }
}

fn validate_keyframes(keyframes: Nbt) -> Result<Nbt, String> {
    let ticks: Vec<i32> = keyframes.as_list().iter().map(|keyframe| keyframe.get("ticks").map_or(0, crate::world::item::component::codec::as_i32)).collect();
    if ticks.is_empty() {
        return Err("Keyframes must not be empty".into());
    }
    if !ticks.is_sorted() {
        return Err("Keyframes must be ordered by ticks field".into());
    }
    if ticks.len() > 1 {
        let mut repeat_count = 0;
        let mut last_ticks = ticks[ticks.len() - 1];
        for &tick in &ticks {
            if tick == last_ticks {
                repeat_count += 1;
                if repeat_count > 2 {
                    return Err(format!("More than 2 keyframes on same tick: {tick}"));
                }
            } else {
                repeat_count = 0;
            }
            last_ticks = tick;
        }
    }
    Ok(keyframes)
}

fn validate_cubic_bezier_controls(controls: Nbt) -> Result<Nbt, String> {
    let control = |index: usize| crate::world::item::component::codec::as_f32(&controls.as_list()[index]);
    if !(0.0..=1.0).contains(&control(0)) {
        return Err("x1 must be in range [0; 1]".into());
    }
    if !(0.0..=1.0).contains(&control(2)) {
        return Err("x2 must be in range [0; 1]".into());
    }
    Ok(controls)
}

fn decode_easing(tag: &Nbt) -> DecodeResult<Nbt> {
    let simple = |tag: &Nbt| match codec::string(tag) {
        Ok(name) if EasingType::by_name(name).is_some() => DecodeResult::success(Nbt::String(name.to_string())),
        Ok(name) => DecodeResult::error(format!("Unknown element id: {name}")),
        Err(error) => DecodeResult::error(error),
    };
    let cubic_bezier = |tag: &Nbt| {
        let controls = Codec::Normalize(Box::new(Codec::SizedList(Box::new(Codec::float()), 4, 4)), validate_cubic_bezier_controls);
        Codec::Record(vec![req("cubic_bezier", (controls, Stream::Nbt))]).decode_partial(tag)
    };
    codec::either_result(simple(tag), || cubic_bezier(tag)).map(|either| either.unwrap_or_else(|easing| easing))
}

fn keyframe_track_fields(attribute: &EnvironmentAttributeInfo, operation: OperationId) -> Vec<Field> {
    let keyframe = Codec::Record(vec![req("ticks", (types::non_negative_int(), Stream::VarInt)), req("value", (argument_codec(attribute, operation), Stream::Nbt))]);
    let keyframes = Codec::Framed(Box::new(Codec::Normalize(Box::new(Codec::list(keyframe)), validate_keyframes)), Wrap::FlatXmap);
    vec![req("keyframes", (keyframes, Stream::Nbt)), def("ease", (Codec::Func(decode_easing), Stream::Nbt), types::s("linear"))]
}

fn decode_modifier(attribute: &EnvironmentAttributeInfo, tag: &Nbt) -> DataResult<OperationId> {
    let name = codec::string(tag)?;
    let Some(operation) = OperationId::by_name(name) else { return Err(format!("Unknown element name:{name}")) };
    if attribute.ty.modifier(operation).is_none() {
        let java_name = OPERATION_NAMES.iter().find(|(known, _)| *known == name).map_or(name, |(_, java_name)| java_name);
        return Err(format!("Unknown element id: {java_name}"));
    }
    Ok(operation)
}

struct TrackCodec(usize);

impl Custom for TrackCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        self.decode_partial(tag).into_result()
    }

    fn decode_partial(&self, tag: &Nbt) -> DecodeResult<Nbt> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let attribute = &ATTRIBUTES[self.0];
        let modifier = match tag.get("modifier") {
            None => DecodeResult::success(OperationId::Override),
            Some(modifier) => match decode_modifier(attribute, modifier) {
                Ok(operation) => DecodeResult::success(operation),
                Err(error) => DecodeResult { value: Some(OperationId::Override), error: Some(error) },
            },
        };
        codec::as_codec(|| {
            modifier.flat_map(|operation| {
                codec::with_frames(crate::util::dfu_frames::key_dispatch, || codec::wrapped(Wrap::MapXmap, || decode_map_codec(&keyframe_track_fields(attribute, operation), tag)))
            })
        })
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Nbt::read(r).map_err(|e| e.to_string())
    }
}

static TRACK_CODECS: LazyLock<Vec<Codec>> = LazyLock::new(|| (0..ATTRIBUTES.len()).map(|id| Codec::Custom(Box::new(TrackCodec(id)))).collect());

fn track_codec_of(key: &Nbt) -> Option<&'static Codec> {
    TRACK_CODECS.get(EnvironmentAttributeInfo::by_name(key.as_str()?)?.id)
}

pub fn tracks_codec() -> Pair {
    (Codec::DispatchedMap(Box::new(types::by_name("minecraft:environment_attribute").0), track_codec_of), Stream::Nbt)
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
            .map(|(ticks, value)| Ok(Nbt::Compound(vec![("ticks".into(), Nbt::Int(*ticks)), ("value".into(), self.modifier.encode_argument(attribute, value)?)].into())))
            .collect::<Result<_, String>>()?;
        fields.push(("keyframes".into(), Nbt::List(keyframes)));
        if self.argument_track.easing_type != EasingType::LINEAR {
            fields.push(("ease".into(), self.argument_track.easing_type.encode()));
        }
        Ok(Nbt::Compound(fields.into()))
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
