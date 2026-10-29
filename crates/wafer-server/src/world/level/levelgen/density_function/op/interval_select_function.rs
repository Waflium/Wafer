use std::cmp::Ordering;

use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{self, Field};
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::density_function::{density_functions, holder_codec};

pub fn fields() -> Vec<Field> {
    vec![
        req("input", holder_codec()),
        req("thresholds", types::list(types::float(density_functions::noise_value_codec()))),
        req("functions", types::list_sized(holder_codec(), 2, i32::MAX as usize)),
    ]
}

pub fn validate(select: &Nbt) -> Result<(), String> {
    let thresholds = select.get("thresholds").map_or(&[][..], Nbt::as_list);
    let functions = select.get("functions").map_or(&[][..], Nbt::as_list);
    if thresholds.len() as i64 != functions.len() as i64 - 1 {
        return Err(format!("Expected {} thresholds for {} functions, but got {}", functions.len() as i64 - 1, functions.len(), thresholds.len()));
    }
    let in_order = thresholds.windows(2).all(|pair| codec::as_f32(&pair[0]).total_cmp(&codec::as_f32(&pair[1])) != Ordering::Greater);
    if in_order { Ok(()) } else { Err("Threshold values must be ordered from smallest to largest".into()) }
}
