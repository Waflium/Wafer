use wafer_core::joml::{AxisAngle4f, Matrix4f, Quaternionf, Vector3f};
use wafer_protocol::nbt::Nbt;

use super::codec::{self, DataResult, DecodeResult};

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

pub fn fixed_size_ints(tag: &Nbt, size: usize) -> DecodeResult<Vec<i32>> {
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
