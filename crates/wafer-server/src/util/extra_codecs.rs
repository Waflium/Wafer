use wafer_core::joml::{AxisAngle4f, Matrix4f, Quaternionf, Vector3f};
use wafer_protocol::nbt::Nbt;

use super::codec::{self, DataResult};

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
