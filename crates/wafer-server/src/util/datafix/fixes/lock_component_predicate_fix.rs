use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::data_component_remainder_fix::DataComponentRemainderFix;

pub struct LockComponentPredicateFix;

impl LockComponentPredicateFix {
    pub fn new(output_schema: &Arc<Schema>) -> DataComponentRemainderFix {
        DataComponentRemainderFix::new(output_schema, "LockComponentPredicateFix", "minecraft:lock", fix_lock)
    }
}

fn escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            c => escaped.push(c),
        }
    }
    escaped
}

pub fn fix_lock(input: Dynamic) -> Option<Dynamic> {
    let name = input.as_string().into_result().ok()?;
    if name.is_empty() {
        return None;
    }
    let name_component = input.create_string(format!("\"{}\"", escape(&name)));
    let components = input.empty_map().set("minecraft:custom_name", name_component);
    Some(input.empty_map().set("components", components))
}
