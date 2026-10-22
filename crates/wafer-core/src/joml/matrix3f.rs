use super::math::fma;
use super::{Matrix4f, Quaternionf};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Matrix3f {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
    pub m20: f32,
    pub m21: f32,
    pub m22: f32,
}

impl Default for Matrix3f {
    fn default() -> Matrix3f {
        Matrix3f { m00: 1.0, m01: 0.0, m02: 0.0, m10: 0.0, m11: 1.0, m12: 0.0, m20: 0.0, m21: 0.0, m22: 1.0 }
    }
}

impl Matrix3f {
    pub fn from_matrix4f(mat: &Matrix4f) -> Matrix3f {
        let m = &mat.m;
        Matrix3f { m00: m[0][0], m01: m[0][1], m02: m[0][2], m10: m[1][0], m11: m[1][1], m12: m[1][2], m20: m[2][0], m21: m[2][1], m22: m[2][2] }
    }

    pub fn scale(self, xyz: f32) -> Matrix3f {
        Matrix3f {
            m00: self.m00 * xyz,
            m01: self.m01 * xyz,
            m02: self.m02 * xyz,
            m10: self.m10 * xyz,
            m11: self.m11 * xyz,
            m12: self.m12 * xyz,
            m20: self.m20 * xyz,
            m21: self.m21 * xyz,
            m22: self.m22 * xyz,
        }
    }

    pub fn transpose(self) -> Matrix3f {
        Matrix3f { m00: self.m00, m01: self.m10, m02: self.m20, m10: self.m01, m11: self.m11, m12: self.m21, m20: self.m02, m21: self.m12, m22: self.m22 }
    }

    pub fn mul(self, right: &Matrix3f) -> Matrix3f {
        Matrix3f {
            m00: fma(self.m00, right.m00, fma(self.m10, right.m01, self.m20 * right.m02)),
            m01: fma(self.m01, right.m00, fma(self.m11, right.m01, self.m21 * right.m02)),
            m02: fma(self.m02, right.m00, fma(self.m12, right.m01, self.m22 * right.m02)),
            m10: fma(self.m00, right.m10, fma(self.m10, right.m11, self.m20 * right.m12)),
            m11: fma(self.m01, right.m10, fma(self.m11, right.m11, self.m21 * right.m12)),
            m12: fma(self.m02, right.m10, fma(self.m12, right.m11, self.m22 * right.m12)),
            m20: fma(self.m00, right.m20, fma(self.m10, right.m21, self.m20 * right.m22)),
            m21: fma(self.m01, right.m20, fma(self.m11, right.m21, self.m21 * right.m22)),
            m22: fma(self.m02, right.m20, fma(self.m12, right.m21, self.m22 * right.m22)),
        }
    }

    pub fn rotate(self, quat: &Quaternionf) -> Matrix3f {
        let w2 = quat.w * quat.w;
        let x2 = quat.x * quat.x;
        let y2 = quat.y * quat.y;
        let z2 = quat.z * quat.z;
        let zw = quat.z * quat.w;
        let dzw = zw + zw;
        let xy = quat.x * quat.y;
        let dxy = xy + xy;
        let xz = quat.x * quat.z;
        let dxz = xz + xz;
        let yw = quat.y * quat.w;
        let dyw = yw + yw;
        let yz = quat.y * quat.z;
        let dyz = yz + yz;
        let xw = quat.x * quat.w;
        let dxw = xw + xw;
        let rm00 = w2 + x2 - z2 - y2;
        let rm01 = dxy + dzw;
        let rm02 = dxz - dyw;
        let rm10 = dxy - dzw;
        let rm11 = y2 - z2 + w2 - x2;
        let rm12 = dyz + dxw;
        let rm20 = dyw + dxz;
        let rm21 = dyz - dxw;
        let rm22 = z2 - y2 - x2 + w2;
        Matrix3f {
            m00: self.m00 * rm00 + self.m10 * rm01 + self.m20 * rm02,
            m01: self.m01 * rm00 + self.m11 * rm01 + self.m21 * rm02,
            m02: self.m02 * rm00 + self.m12 * rm01 + self.m22 * rm02,
            m10: self.m00 * rm10 + self.m10 * rm11 + self.m20 * rm12,
            m11: self.m01 * rm10 + self.m11 * rm11 + self.m21 * rm12,
            m12: self.m02 * rm10 + self.m12 * rm11 + self.m22 * rm12,
            m20: self.m00 * rm20 + self.m10 * rm21 + self.m20 * rm22,
            m21: self.m01 * rm20 + self.m11 * rm21 + self.m21 * rm22,
            m22: self.m02 * rm20 + self.m12 * rm21 + self.m22 * rm22,
        }
    }
}
