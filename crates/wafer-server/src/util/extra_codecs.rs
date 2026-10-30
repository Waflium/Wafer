use wafer_core::joml::{AxisAngle4f, Matrix4f, Quaternionf, Vector3f};
use wafer_protocol::nbt::Nbt;

use super::argb;
use super::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::{self as component_codec, Codec};
use crate::world::item::component::types::{self, Pair};

pub fn fixed_size_list_partial<T>(tag: &Nbt, size: usize, decode: impl Fn(&Nbt) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    let result = codec::list_partial(tag, decode);
    let Some(mut values) = result.value else { return result };
    let size_error = (values.len() != size).then(|| format!("Input is not a list of {size} elements"));
    let value = (values.len() >= size).then(|| {
        values.truncate(size);
        values
    });
    let error = match (result.error, size_error) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (first, second) => first.or(second),
    };
    DecodeResult { value, error }
}

const INT_STREAM_FRAMES: [&str; 14] = [
    "com.mojang.serialization.DynamicOps.lambda$getIntStream$16(DynamicOps.java:169)",
    "java.base/java.util.stream.ReferencePipeline$4$1.accept(ReferencePipeline.java:231)",
    "java.base/java.util.AbstractList$RandomAccessSpliterator.tryAdvance(AbstractList.java:708)",
    "java.base/java.util.stream.ReferencePipeline.forEachWithCancel(ReferencePipeline.java:147)",
    "java.base/java.util.stream.AbstractPipeline.copyIntoWithCancel(AbstractPipeline.java:588)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:574)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:635)",
    "java.base/java.util.stream.AbstractPipeline.evaluateToArrayNode(AbstractPipeline.java:291)",
    "java.base/java.util.stream.IntPipeline.toArray(IntPipeline.java:564)",
    "net.minecraft.util.Util.fixedSize(Util.java:962)",
    "com.mojang.serialization.Decoder$1.lambda$decode$1(Decoder.java:49)",
    "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)",
    "com.mojang.serialization.Decoder$1.decode(Decoder.java:49)",
];

const VEC3I_FIXED_SIZE: &str = "net.minecraft.core.Vec3i.lambda$static$0(Vec3i.java:19)";
pub const BLOCK_POS_FIXED_SIZE: &str = "net.minecraft.core.BlockPos.lambda$static$0(BlockPos.java:35)";

fn throw_int_stream_element(value: &Nbt, owner: &'static str) {
    let Nbt::LazilyParsedNumber(text) = value else { return };
    let Some(message) = wafer_protocol::lazily_parsed_number::number_limits_error(text) else { return };
    let mut frames: Vec<&'static str> = vec![
        "com.google.gson.internal.NumberLimits.parseBigDecimal(NumberLimits.java:27)",
        "com.google.gson.internal.LazilyParsedNumber.asBigDecimal(LazilyParsedNumber.java:41)",
        "com.google.gson.internal.LazilyParsedNumber.intValue(LazilyParsedNumber.java:52)",
    ];
    frames.extend_from_slice(&INT_STREAM_FRAMES[..10]);
    frames.push(INT_STREAM_FRAMES[10]);
    frames.push(owner);
    frames.extend_from_slice(&INT_STREAM_FRAMES[11..]);
    frames.push("com.mojang.serialization.Codec$2.decode(Codec.java:74)");
    crate::core::data_registries::throw_unwinding("java.lang.NumberFormatException", message, &frames);
}

pub fn fixed_size_ints(tag: &Nbt, size: usize) -> DecodeResult<Vec<i32>> {
    fixed_size_ints_owned(tag, size, VEC3I_FIXED_SIZE)
}

pub fn fixed_size_ints_owned(tag: &Nbt, size: usize, owner: &'static str) -> DecodeResult<Vec<i32>> {
    let numeric = |value: &Nbt| match *value {
        Nbt::Byte(v) => Some(v as i32),
        Nbt::Short(v) => Some(v as i32),
        Nbt::Int(v) => Some(v),
        Nbt::Long(v) => Some(v as i32),
        Nbt::Float(v) => Some(wafer_core::mth::floor_f(v)),
        Nbt::Double(v) => Some(wafer_core::mth::floor(v)),
        _ => None,
    };
    let mut ints: Vec<i32> = match tag {
        Nbt::List(values) if crate::nbt::is_json_source() => {
            let json_numeric = |value: &Nbt| match value {
                Nbt::LazilyParsedNumber(text) => Some(wafer_protocol::lazily_parsed_number::int_value(text)),
                Nbt::Long(v) => Some(*v as i32),
                Nbt::Double(v) => Some(if v.abs() >= 2f64.powi(84) { 0 } else { *v as i128 as i32 }),
                _ => None,
            };
            if values.iter().any(|value| json_numeric(value).is_none()) {
                return DecodeResult::error(format!("Some elements are not ints: {}", crate::nbt::to_string(tag)));
            }
            values
                .iter()
                .map(|value| {
                    throw_int_stream_element(value, owner);
                    json_numeric(value).expect("a number")
                })
                .collect()
        }
        _ if crate::nbt::is_json_source() => return DecodeResult::error(format!("Not a json array: {}", crate::nbt::to_string(tag))),
        Nbt::IntArray(values) => values.clone(),
        Nbt::ByteArray(values) => values.iter().map(|&v| v as i32).collect(),
        Nbt::LongArray(values) => values.iter().map(|&v| v as i32).collect(),
        Nbt::List(values) => match values.iter().map(numeric).collect::<Option<Vec<i32>>>() {
            Some(ints) => ints,
            None => return DecodeResult::error(format!("Some elements are not ints: {}", crate::nbt::to_string(tag))),
        },
        _ => return DecodeResult::error("Not a list"),
    };
    if ints.len() == size {
        return DecodeResult::success(ints);
    }
    let error = Some(format!("Input is not a list of {size} ints"));
    if ints.len() > size {
        ints.truncate(size);
        DecodeResult { value: Some(ints), error }
    } else {
        DecodeResult { value: None, error }
    }
}

fn decode_vec3i(tag: &Nbt) -> DecodeResult<Nbt> {
    fixed_size_ints(tag, 3).map(Nbt::IntArray)
}

fn decode_offset(tag: &Nbt) -> DecodeResult<Nbt> {
    codec::wrapped(component_codec::Wrap::FlatXmap, || {
        fixed_size_ints(tag, 3).flat_map(|ints| {
            if ints.iter().all(|value| value.wrapping_abs() <= 16) {
                DecodeResult::success(Nbt::IntArray(ints))
            } else {
                DecodeResult::error(format!("Position out of range, expected at most 16: Vec3i{{x={}, y={}, z={}}}", ints[0], ints[1], ints[2]))
            }
        })
    })
}

pub fn offset_codec() -> Pair {
    (Codec::Func(decode_offset), crate::world::item::component::codec::Stream::Nbt)
}

pub fn vec3i_codec() -> Pair {
    (Codec::Func(decode_vec3i), crate::world::item::component::codec::Stream::Nbt)
}

pub fn fixed_size_longs(tag: &Nbt, size: usize) -> DecodeResult<Vec<i64>> {
    let numeric = |value: &Nbt| match *value {
        Nbt::Byte(v) => Some(v as i64),
        Nbt::Short(v) => Some(v as i64),
        Nbt::Int(v) => Some(v as i64),
        Nbt::Long(v) => Some(v),
        Nbt::Float(v) => Some(v as i64),
        Nbt::Double(v) => Some(v as i64),
        _ => None,
    };
    let mut longs: Vec<i64> = match tag {
        Nbt::LongArray(values) => values.clone(),
        Nbt::IntArray(values) => values.iter().map(|&v| v as i64).collect(),
        Nbt::ByteArray(values) => values.iter().map(|&v| v as i64).collect(),
        Nbt::List(values) => match values.iter().map(numeric).collect::<Option<Vec<i64>>>() {
            Some(longs) => longs,
            None => return DecodeResult::error(format!("Some elements are not longs: {}", crate::nbt::to_string(tag))),
        },
        _ => return DecodeResult::error("Not a list"),
    };
    if longs.len() == size {
        return DecodeResult::success(longs);
    }
    let error = Some(format!("Input is not a list of {size} longs"));
    if longs.len() > size {
        longs.truncate(size);
        DecodeResult { value: Some(longs), error }
    } else {
        DecodeResult { value: None, error }
    }
}

pub fn uuid(tag: &Nbt) -> DecodeResult<u128> {
    fixed_size_ints(tag, 4).map(|ints| ints.iter().fold(0u128, |acc, &v| acc << 32 | v as u32 as u128))
}

pub fn block_pos(tag: &Nbt) -> DecodeResult<wafer_core::BlockPos> {
    if let Nbt::IntArray(ints) = tag
        && let [x, y, z] = ints.as_slice()
    {
        return DecodeResult::success(wafer_core::BlockPos::new(*x, *y, *z));
    }
    fixed_size_ints(tag, 3).map(|ints| wafer_core::BlockPos::new(ints[0], ints[1], ints[2]))
}

pub fn vec3(tag: &Nbt) -> DecodeResult<wafer_core::Vec3> {
    if let Nbt::List(items) = tag
        && let [x, y, z] = items.as_slice()
        && let (Ok(x), Ok(y), Ok(z)) = (codec::double(x), codec::double(y), codec::double(z))
    {
        return DecodeResult::success(wafer_core::Vec3::new(x, y, z));
    }
    fixed_size_list_partial(tag, 3, |element| codec::double(element).into()).map(|values| wafer_core::Vec3::new(values[0], values[1], values[2]))
}

pub fn vec2(tag: &Nbt) -> DecodeResult<(f32, f32)> {
    if let Nbt::List(items) = tag
        && let [x, y] = items.as_slice()
        && let (Ok(x), Ok(y)) = (codec::float(x), codec::float(y))
    {
        return DecodeResult::success((x, y));
    }
    fixed_size_list_partial(tag, 2, |element| codec::float(element).into()).map(|values| (values[0], values[1]))
}

fn fixed_size_floats(tag: &Nbt, size: usize) -> DataResult<Vec<f32>> {
    let values = codec::list_of(tag, codec::float)?;
    if values.len() < size {
        return Err(format!("Input is not a list of {size} elements"));
    }
    Ok(values[..size].to_vec())
}

pub fn vector3f(tag: &Nbt) -> DataResult<Vector3f> {
    let d = fixed_size_floats(tag, 3)?;
    Ok(Vector3f::new(d[0], d[1], d[2]))
}

pub fn vector3f_to_nbt(vec: Vector3f) -> Nbt {
    Nbt::List(vec![Nbt::Float(vec.x), Nbt::Float(vec.y), Nbt::Float(vec.z)])
}

fn quaternionf_components(tag: &Nbt) -> DataResult<Quaternionf> {
    let d = fixed_size_floats(tag, 4)?;
    Ok(Quaternionf::new(d[0], d[1], d[2], d[3]).normalize())
}

fn axisangle4f(tag: &Nbt) -> DataResult<AxisAngle4f> {
    let angle = codec::decode_field(tag, "angle", codec::float)?;
    let axis = codec::decode_field(tag, "axis", vector3f)?;
    Ok(AxisAngle4f::new(angle, axis))
}

pub fn quaternionf(tag: &Nbt) -> DataResult<Quaternionf> {
    Ok(match codec::either(tag, quaternionf_components, axisangle4f)? {
        Ok(quaternion) => quaternion,
        Err(axis_angle) => Quaternionf::from_axis_angle(axis_angle),
    })
}

pub fn quaternionf_to_nbt(q: Quaternionf) -> Nbt {
    Nbt::List(vec![Nbt::Float(q.x), Nbt::Float(q.y), Nbt::Float(q.z), Nbt::Float(q.w)])
}

pub fn matrix4f(tag: &Nbt) -> DataResult<Matrix4f> {
    let l = fixed_size_floats(tag, 16)?;
    let mut result = Matrix4f::default();
    for (i, value) in l.into_iter().enumerate() {
        result.set_row_column(i >> 2, i & 3, value);
    }
    Ok(result)
}

fn parse_hex_color(tag: &Nbt, digits: usize) -> Result<i32, String> {
    let string = component_codec::string(tag)?;
    let hex = string.strip_prefix('#').ok_or("Hex color must begin with #")?;
    if hex.len() != digits {
        return Err(format!("Hex color is wrong size, expected {digits} digits but got {}", hex.len()));
    }
    u32::from_str_radix(hex, 16).map(|value| value as i32).map_err(|_| format!("Invalid color value: {string}"))
}

fn color_floats(tag: &Nbt, size: usize) -> Result<Vec<f32>, String> {
    let items = component_codec::list(tag)?;
    if items.len() != size {
        return Err(format!("Input is not a list of {size} elements"));
    }
    items.iter().map(|item| Codec::float().decode(item).map(|value| component_codec::as_f32(&value))).collect()
}

fn color_int(tag: &Nbt) -> Result<i32, String> {
    Codec::int().decode(tag).map(|value| component_codec::as_i32(&value))
}

pub fn hex_color(color: i32, digits: usize) -> String {
    format!("#{:0width$x}", color as u32 as u64 & ((1u64 << (digits * 4)) - 1), width = digits)
}

fn decode_string_rgb_color(tag: Nbt) -> Result<Nbt, String> {
    let color = match &tag {
        Nbt::String(_) => argb::opaque(parse_hex_color(&tag, 6)?),
        Nbt::List(_) => {
            let channels = color_floats(&tag, 3)?;
            argb::color_from_float(1.0, channels[0], channels[1], channels[2])
        }
        other => color_int(other)?,
    };
    Ok(Nbt::String(hex_color(argb::transparent(color), 6)))
}

fn check_string_rgb_hex(value: Nbt) -> Result<Nbt, String> {
    parse_hex_color(&value, 6)?;
    Ok(value)
}

pub fn string_rgb_color() -> Pair {
    let hex = Codec::Normalize(Box::new(Codec::string()), check_string_rgb_hex);
    let numeric = Codec::Either(Box::new(Codec::int()), Box::new(types::vector(3)));
    (Codec::Normalize(Box::new(Codec::Either(Box::new(hex), Box::new(numeric))), decode_string_rgb_color), types::transform(decode_string_rgb_color).1)
}

fn check_non_empty_holder_set(value: Nbt) -> Result<Nbt, String> {
    match &value {
        Nbt::List(items) if items.is_empty() => Err("List must have contents".into()),
        _ => Ok(value),
    }
}

pub fn non_empty_holder_set(list_codec: Pair) -> Pair {
    (Codec::Framed(Box::new(Codec::Normalize(Box::new(list_codec.0), check_non_empty_holder_set)), component_codec::Wrap::FlatXmap), list_codec.1)
}
