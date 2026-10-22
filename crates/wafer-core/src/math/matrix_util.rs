use std::sync::LazyLock;

use super::GivensParameters;
use crate::joml::math::sqrt;
use crate::joml::{Matrix3f, Matrix4f, Quaternionf, Vector3f};

static G: LazyLock<f32> = LazyLock::new(|| 3.0 + 2.0 * sqrt(2.0));
static PI_4: LazyLock<GivensParameters> = LazyLock::new(|| GivensParameters::from_positive_angle((std::f64::consts::PI / 4.0) as f32));

fn approx_givens_quat(a11: f32, a12: f32, a22: f32) -> GivensParameters {
    let ch = 2.0 * (a11 - a22);
    let sh = a12;
    if *G * sh * sh < ch * ch { GivensParameters::from_unnormalized(sh, ch) } else { *PI_4 }
}

fn qr_givens_quat(a1: f32, a2: f32) -> GivensParameters {
    let p = (a1 as f64).hypot(a2 as f64) as f32;
    let mut sh = if p > 1.0e-6 { a2 } else { 0.0 };
    let mut ch = a1.abs() + if p.is_nan() { p } else { p.max(1.0e-6) };
    if a1 < 0.0 {
        std::mem::swap(&mut sh, &mut ch);
    }
    GivensParameters::from_unnormalized(sh, ch)
}

fn similarity_transform(a: Matrix3f, q: Matrix3f) -> Matrix3f {
    let a = a.mul(&q);
    q.transpose().mul(&a)
}

fn step_jacobi(m: &mut Matrix3f, output: &mut Quaternionf) {
    if m.m01 * m.m01 + m.m10 * m.m10 > 1.0e-6 {
        let p = approx_givens_quat(m.m00, 0.5 * (m.m01 + m.m10), m.m11);
        *output = output.mul(p.around_z_quaternion());
        *m = similarity_transform(*m, p.around_z());
    }
    if m.m02 * m.m02 + m.m20 * m.m20 > 1.0e-6 {
        let p = approx_givens_quat(m.m00, 0.5 * (m.m02 + m.m20), m.m22).inverse();
        *output = output.mul(p.around_y_quaternion());
        *m = similarity_transform(*m, p.around_y());
    }
    if m.m12 * m.m12 + m.m21 * m.m21 > 1.0e-6 {
        let p = approx_givens_quat(m.m11, 0.5 * (m.m12 + m.m21), m.m22);
        *output = output.mul(p.around_x_quaternion());
        *m = similarity_transform(*m, p.around_x());
    }
}

pub fn eigenvalue_jacobi(in_out: &mut Matrix3f, steps: i32) -> Quaternionf {
    let mut result = Quaternionf::IDENTITY;
    for _ in 0..steps {
        step_jacobi(in_out, &mut result);
    }
    result.normalize()
}

pub fn svd_decompose(input: &Matrix4f) -> (Vector3f, Quaternionf, Vector3f, Quaternionf) {
    let scale_factor = 1.0 / input.m33();
    let (u, s, v) = svd_decompose3(Matrix3f::from_matrix4f(input).scale(scale_factor));
    (input.get_translation().mul(scale_factor), u, s, v)
}

fn svd_decompose3(input: Matrix3f) -> (Quaternionf, Vector3f, Quaternionf) {
    let mut scratch = input.transpose().mul(&input);
    let v = eigenvalue_jacobi(&mut scratch, 5);
    let column_scale_square0 = scratch.m00;
    let column_scale_square1 = scratch.m11;
    let zero_column0 = (column_scale_square0 as f64) < 1.0e-6;
    let zero_column1 = (column_scale_square1 as f64) < 1.0e-6;
    let u012s = input.rotate(&v);
    let mut u = Quaternionf::IDENTITY;
    let p = if zero_column0 { qr_givens_quat(u012s.m11, -u012s.m10) } else { qr_givens_quat(u012s.m00, u012s.m01) };
    u = u.mul(p.around_z_quaternion());
    let u12s = p.around_z().transpose().mul(&u012s);
    let p = if zero_column0 { qr_givens_quat(u12s.m22, -u12s.m20) } else { qr_givens_quat(u12s.m00, u12s.m02) }.inverse();
    u = u.mul(p.around_y_quaternion());
    let u2s = p.around_y().transpose().mul(&u12s);
    let p = if zero_column1 { qr_givens_quat(u2s.m22, -u2s.m21) } else { qr_givens_quat(u2s.m11, u2s.m12) };
    u = u.mul(p.around_x_quaternion());
    let sm = p.around_x().transpose().mul(&u2s);
    (u, Vector3f::new(sm.m00, sm.m11, sm.m22), v.conjugate())
}
