use serde_json::Value;

pub struct GeodeCrackSettings {
    pub generate_crack_chance: f64,
    pub base_crack_size: f64,
    pub crack_point_offset: i32,
}

impl GeodeCrackSettings {
    pub fn parse(json: &Value) -> GeodeCrackSettings {
        GeodeCrackSettings {
            generate_crack_chance: json.get("generate_crack_chance").and_then(Value::as_f64).unwrap_or(1.0),
            base_crack_size: json.get("base_crack_size").and_then(Value::as_f64).unwrap_or(2.0),
            crack_point_offset: json.get("crack_point_offset").and_then(Value::as_i64).map_or(2, |v| v as i32),
        }
    }
}

pub fn codec() -> crate::world::item::component::types::Pair {
    use super::feature::geode_feature;
    use crate::util::range_codecs::{double_range, int_range};
    use crate::world::item::component::types::{def, i, record};
    use wafer_protocol::nbt::Nbt;
    record(vec![
        def("generate_crack_chance", geode_feature::chance_range(), Nbt::Double(1.0)),
        def("base_crack_size", double_range(0.0, 5.0), Nbt::Double(2.0)),
        def("crack_point_offset", int_range(0, 10), i(2)),
    ])
}
