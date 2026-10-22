use super::Vector3f;

const PI2_F: f32 = (std::f64::consts::PI * 2.0) as f32;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AxisAngle4f {
    pub angle: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl AxisAngle4f {
    pub fn new(angle: f32, v: Vector3f) -> AxisAngle4f {
        let angle = (if (angle as f64) < 0.0 { PI2_F + angle % PI2_F } else { angle }) % PI2_F;
        AxisAngle4f { angle, x: v.x, y: v.y, z: v.z }
    }
}
