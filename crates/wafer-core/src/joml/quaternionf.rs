use super::{AxisAngle4f, Vector3f};
use super::math::{cos_from_sin, float_to_int_bits, fma, fma_f64, invsqrt, sin};

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

    pub fn rotation_x(angle: f32) -> Quaternionf {
        let sin = sin(angle * 0.5);
        let cos = cos_from_sin(sin, angle * 0.5);
        Quaternionf { x: sin, y: 0.0, z: 0.0, w: cos }
    }

    pub fn rotation_xyz(angle_x: f32, angle_y: f32, angle_z: f32) -> Quaternionf {
        let sx = sin(angle_x * 0.5);
        let cx = cos_from_sin(sx, angle_x * 0.5);
        let sy = sin(angle_y * 0.5);
        let cy = cos_from_sin(sy, angle_y * 0.5);
        let sz = sin(angle_z * 0.5);
        let cz = cos_from_sin(sz, angle_z * 0.5);
        let (cycz, sysz, sycz, cysz) = (cy * cz, sy * sz, sy * cz, cy * sz);
        Quaternionf { x: sx * cycz + cx * sysz, y: cx * sycz - sx * cysz, z: cx * cysz + sx * sycz, w: cx * cycz - sx * sysz }
    }

    pub fn transform(self, v: Vector3f) -> Vector3f {
        let (x, y, z) = (v.x, v.y, v.z);
        let (xx, yy, zz, ww) = (self.x * self.x, self.y * self.y, self.z * self.z, self.w * self.w);
        let (xy, xz, yz, xw) = (self.x * self.y, self.x * self.z, self.y * self.z, self.x * self.w);
        let (zw, yw) = (self.z * self.w, self.y * self.w);
        let k = 1.0 / (xx + yy + zz + ww);
        Vector3f::new(
            fma((xx - yy - zz + ww) * k, x, fma(2.0 * (xy - zw) * k, y, (2.0 * (xz + yw) * k) * z)),
            fma(2.0 * (xy + zw) * k, x, fma((yy - xx - zz + ww) * k, y, (2.0 * (yz - xw) * k) * z)),
            fma(2.0 * (xz - yw) * k, x, fma(2.0 * (yz + xw) * k, y, ((zz - xx - yy + ww) * k) * z)),
        )
    }

    pub fn rotation_y(angle: f32) -> Quaternionf {
        let sin = sin(angle * 0.5);
        let cos = cos_from_sin(sin, angle * 0.5);
        Quaternionf { x: 0.0, y: sin, z: 0.0, w: cos }
    }

    pub fn rotate_x(self, angle: f32) -> Quaternionf {
        let sin = sin(angle * 0.5);
        let cos = cos_from_sin(sin, angle * 0.5);
        Quaternionf { x: self.w * sin + self.x * cos, y: self.y * cos + self.z * sin, z: self.z * cos - self.y * sin, w: self.w * cos - self.x * sin }
    }

    pub fn transform_f64(self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let (xx, yy, zz, ww) = (self.x * self.x, self.y * self.y, self.z * self.z, self.w * self.w);
        let (xy, xz, yz, xw) = (self.x * self.y, self.x * self.z, self.y * self.z, self.x * self.w);
        let (zw, yw) = (self.z * self.w, self.y * self.w);
        let k = 1.0 / (xx + yy + zz + ww);
        (
            fma_f64(((xx - yy - zz + ww) * k) as f64, x, fma_f64((2.0 * (xy - zw) * k) as f64, y, (2.0 * (xz + yw) * k) as f64 * z)),
            fma_f64((2.0 * (xy + zw) * k) as f64, x, fma_f64(((yy - xx - zz + ww) * k) as f64, y, (2.0 * (yz - xw) * k) as f64 * z)),
            fma_f64((2.0 * (xz - yw) * k) as f64, x, fma_f64((2.0 * (yz + xw) * k) as f64, y, ((zz - xx - yy + ww) * k) as f64 * z)),
        )
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

#[cfg(test)]
mod tests {
    use super::Quaternionf;
    use crate::mth::DEG_TO_RAD;

    #[test]
    fn view_quaternion_transforms_match_joml() {
        let rotations = [(0.0f32, 0.0f32), (30.0, 45.0), (-89.5, 173.2), (12.3, -271.7), (90.0, 0.0)];
        let expected: [[i64; 6]; 5] = [
            [0, 0, 4607182418800017408, 4599075939470750515, -4618891777831180698, 4612811918334230528],
            [-4619681056562348032, -4620693218218999808, 4603690979755556864, -4615868458519756800, -4612333556352417792, 4609414293334314188],
            [-4660964102896615424, 4607182075739504640, -4647223265029259264, -4622794548757659648, 4612797948589454131, -4619061193990576538],
            [-4616400247009050624, -4626528788136591360, 4584011705893257216, -4611047445927821312, -4615214553272156160, 4600299254980385178],
            [0, -4616189618054758400, 0, 4599075939470750515, -4610560118520545280, -4618891777831180698],
        ];
        for ((x_rot, y_rot), expected) in rotations.into_iter().zip(expected) {
            let q = Quaternionf::rotation_y(-y_rot * DEG_TO_RAD).rotate_x(x_rot * DEG_TO_RAD);
            let (dx, dy, dz) = q.transform_f64(0.0, 0.0, 1.0);
            let (ex, ey, ez) = q.transform_f64(0.3, -0.7, 2.5);
            assert_eq!([dx, dy, dz, ex, ey, ez].map(|v| v.to_bits() as i64), expected);
        }
    }
}
