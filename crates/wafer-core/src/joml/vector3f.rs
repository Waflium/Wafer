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

    pub fn mul(self, scalar: f32) -> Vector3f {
        Vector3f::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

impl PartialEq for Vector3f {
    fn eq(&self, other: &Vector3f) -> bool {
        float_to_int_bits(self.x) == float_to_int_bits(other.x) && float_to_int_bits(self.y) == float_to_int_bits(other.y) && float_to_int_bits(self.z) == float_to_int_bits(other.z)
    }
}
