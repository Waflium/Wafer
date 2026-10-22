use super::AxisAngle4f;
use super::math::{cos_from_sin, float_to_int_bits, fma, invsqrt, sin};

#[derive(Clone, Copy, Debug)]
pub struct Quaternionf {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quaternionf {
    fn default() -> Quaternionf {
        Quaternionf::IDENTITY
    }
}

impl Quaternionf {
    pub const IDENTITY: Quaternionf = Quaternionf { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Quaternionf {
        Quaternionf { x, y, z, w }
    }

    pub fn from_axis_angle(axis_angle: AxisAngle4f) -> Quaternionf {
        let sin = sin(axis_angle.angle * 0.5);
        let cos = cos_from_sin(sin, axis_angle.angle * 0.5);
        Quaternionf { x: axis_angle.x * sin, y: axis_angle.y * sin, z: axis_angle.z * sin, w: cos }
    }

    pub fn normalize(self) -> Quaternionf {
        let inv_norm = invsqrt(fma(self.x, self.x, fma(self.y, self.y, fma(self.z, self.z, self.w * self.w))));
        Quaternionf { x: self.x * inv_norm, y: self.y * inv_norm, z: self.z * inv_norm, w: self.w * inv_norm }
    }

    pub fn mul(self, q: Quaternionf) -> Quaternionf {
        Quaternionf {
            x: fma(self.w, q.x, fma(self.x, q.w, fma(self.y, q.z, -self.z * q.y))),
            y: fma(self.w, q.y, fma(-self.x, q.z, fma(self.y, q.w, self.z * q.x))),
            z: fma(self.w, q.z, fma(self.x, q.y, fma(-self.y, q.x, self.z * q.w))),
            w: fma(self.w, q.w, fma(-self.x, q.x, fma(-self.y, q.y, -self.z * q.z))),
        }
    }

    pub fn conjugate(self) -> Quaternionf {
        Quaternionf { x: -self.x, y: -self.y, z: -self.z, w: self.w }
    }
}

impl PartialEq for Quaternionf {
    fn eq(&self, other: &Quaternionf) -> bool {
        float_to_int_bits(self.x) == float_to_int_bits(other.x) && float_to_int_bits(self.y) == float_to_int_bits(other.y) && float_to_int_bits(self.z) == float_to_int_bits(other.z) && float_to_int_bits(self.w) == float_to_int_bits(other.w)
    }
}
