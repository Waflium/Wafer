use super::math::fma;
use super::math::float_to_int_bits;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3f {
    pub const fn new(x: f32, y: f32, z: f32) -> Vector3f {
        Vector3f { x, y, z }
    }

    pub fn cross(self, v: Vector3f) -> Vector3f {
        let rx = fma(self.y, v.z, -self.z * v.y);
        let ry = fma(self.z, v.x, -self.x * v.z);
        let rz = fma(self.x, v.y, -self.y * v.x);
        Vector3f::new(rx, ry, rz)
    }

    pub fn dot(self, v: Vector3f) -> f32 {
        fma(self.x, v.x, fma(self.y, v.y, self.z * v.z))
    }

    pub fn mul(self, scalar: f32) -> Vector3f {
        Vector3f::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

impl PartialEq for Vector3f {
    fn eq(&self, other: &Vector3f) -> bool {
        float_to_int_bits(self.x) == float_to_int_bits(other.x) && float_to_int_bits(self.y) == float_to_int_bits(other.y) && float_to_int_bits(self.z) == float_to_int_bits(other.z)
    }
}
