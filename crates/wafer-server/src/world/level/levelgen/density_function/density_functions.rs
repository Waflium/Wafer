use wafer_protocol::nbt::Nbt;

use super::generator::{constant_function, distance_to_point_function, end_island_function, gradient_function, noise_function, shift_noise_function, simple_density_function};
use super::op::{
    binary_function, blend_density_function, cache_function, clamp_function, find_top_surface_function, interpolated_function, interval_select_function, lerp_function,
    pow_function, range_choice_function, round_function, slice_function, spline_function, unary_function,
};
use crate::world::item::component::codec::{Codec, Dispatch, RangeError, Selector, Stream};
use crate::world::item::component::types::Pair;
use crate::world::level::levelgen::synth::blended_noise;

pub const AXES: &[&str] = &["x", "y", "z"];

pub fn noise_value_codec() -> Codec {
    Codec::Float(-1000000.0, 1000000.0, false, RangeError::Outside)
}

fn validate(function: Nbt) -> Result<Nbt, String> {
    match function.get("type").and_then(Nbt::as_str) {
        Some("minecraft:gradient") => gradient_function::validate(&function)?,
        Some("minecraft:clamp") => clamp_function::validate(&function)?,
        Some("minecraft:interval_select") => interval_select_function::validate(&function)?,
        _ => {}
    }
    Ok(function)
}

fn full_direct_codec() -> Codec {
    let mut cases = vec![
        ("minecraft:constant", constant_function::fields()),
        (simple_density_function::BLEND_ALPHA, Vec::new()),
        (simple_density_function::BLEND_OFFSET, Vec::new()),
        (simple_density_function::BEARDIFIER, Vec::new()),
        ("minecraft:noise", noise_function::fields()),
        ("minecraft:end_outer_islands", end_island_function::fields()),
        ("minecraft:distance_to_point", distance_to_point_function::fields()),
        ("minecraft:gradient", gradient_function::fields()),
        ("minecraft:shift_a", shift_noise_function::fields()),
        ("minecraft:shift_b", shift_noise_function::fields()),
        ("minecraft:shift", shift_noise_function::fields()),
    ];
    cases.extend(unary_function::TYPES.iter().map(|id| (*id, unary_function::fields())));
    cases.extend(round_function::TYPES.iter().map(|id| (*id, round_function::fields())));
    cases.extend(binary_function::TYPES.iter().map(|id| (*id, binary_function::fields())));
    cases.extend([
        ("minecraft:pow", pow_function::fields()),
        ("minecraft:spline", spline_function::fields()),
        ("minecraft:lerp", lerp_function::fields()),
        ("minecraft:clamp", clamp_function::fields()),
        ("minecraft:range_choice", range_choice_function::fields()),
        ("minecraft:interval_select", interval_select_function::fields()),
        ("minecraft:cache", cache_function::fields()),
        ("minecraft:blend_density", blend_density_function::fields()),
        ("minecraft:interpolated", interpolated_function::fields()),
        ("minecraft:slice", slice_function::fields()),
        ("minecraft:find_top_surface", find_top_surface_function::fields()),
        ("minecraft:old_blended_noise", blended_noise::fields()),
    ]);
    Codec::Normalize(
        Box::new(Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:worldgen/density_function_type"), cases }))),
        validate,
    )
}

pub fn direct_codec() -> Pair {
    (Codec::Either(Box::new(noise_value_codec()), Box::new(full_direct_codec())), Stream::Nbt)
}
