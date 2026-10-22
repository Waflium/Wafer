const PI_F: f32 = std::f64::consts::PI as f32;
const PI2_F: f32 = (std::f64::consts::PI * 2.0) as f32;
const PI_HALF_F: f32 = (std::f64::consts::PI / 2.0) as f32;

pub fn sin(rad: f32) -> f32 {
    (rad as f64).sin() as f32
}

pub fn sqrt(r: f32) -> f32 {
    (r as f64).sqrt() as f32
}

pub fn invsqrt(r: f32) -> f32 {
    1.0 / (r as f64).sqrt() as f32
}

pub fn fma(a: f32, b: f32, c: f32) -> f32 {
    a * b + c
}

pub fn cos_from_sin(sin: f32, angle: f32) -> f32 {
    let cos = sqrt(1.0 - sin * sin);
    let a = angle + PI_HALF_F;
    let mut b = a - (a / PI2_F) as i32 as f32 * PI2_F;
    if (b as f64) < 0.0 {
        b += PI2_F;
    }
    if b >= PI_F { -cos } else { cos }
}

pub fn float_to_int_bits(value: f32) -> u32 {
    if value.is_nan() { 0x7fc00000 } else { value.to_bits() }
}
