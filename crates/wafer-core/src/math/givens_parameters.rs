use crate::joml::math::{cos_from_sin, invsqrt, sin};
use crate::joml::{Matrix3f, Quaternionf};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GivensParameters {
    pub sin_half: f32,
    pub cos_half: f32,
}

impl GivensParameters {
    pub fn from_unnormalized(sin_half: f32, cos_half: f32) -> GivensParameters {
        let w = invsqrt(sin_half * sin_half + cos_half * cos_half);
        GivensParameters { sin_half: w * sin_half, cos_half: w * cos_half }
    }

    pub fn from_positive_angle(angle: f32) -> GivensParameters {
        let sin = sin(angle / 2.0);
        let cos = cos_from_sin(sin, angle / 2.0);
        GivensParameters { sin_half: sin, cos_half: cos }
    }

    pub fn inverse(self) -> GivensParameters {
        GivensParameters { sin_half: -self.sin_half, cos_half: self.cos_half }
    }

    pub fn around_x_quaternion(self) -> Quaternionf {
        Quaternionf::new(self.sin_half, 0.0, 0.0, self.cos_half)
    }

    pub fn around_y_quaternion(self) -> Quaternionf {
        Quaternionf::new(0.0, self.sin_half, 0.0, self.cos_half)
    }

    pub fn around_z_quaternion(self) -> Quaternionf {
        Quaternionf::new(0.0, 0.0, self.sin_half, self.cos_half)
    }

    pub fn cos(self) -> f32 {
        self.cos_half * self.cos_half - self.sin_half * self.sin_half
    }

    pub fn sin(self) -> f32 {
        2.0 * self.sin_half * self.cos_half
    }

    pub fn around_x(self) -> Matrix3f {
        let (c, s) = (self.cos(), self.sin());
        Matrix3f { m00: 1.0, m01: 0.0, m02: 0.0, m10: 0.0, m11: c, m12: s, m20: 0.0, m21: -s, m22: c }
    }

    pub fn around_y(self) -> Matrix3f {
        let (c, s) = (self.cos(), self.sin());
        Matrix3f { m00: c, m01: 0.0, m02: -s, m10: 0.0, m11: 1.0, m12: 0.0, m20: s, m21: 0.0, m22: c }
    }

    pub fn around_z(self) -> Matrix3f {
        let (c, s) = (self.cos(), self.sin());
        Matrix3f { m00: c, m01: s, m02: 0.0, m10: -s, m11: c, m12: 0.0, m20: 0.0, m21: 0.0, m22: 1.0 }
    }
}
