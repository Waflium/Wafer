use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::data_component_remainder_fix::DataComponentRemainderFix;

const INVALID_LOCK_CUSTOM_NAME: &str = "\"\"";

pub struct InvalidLockComponentFix;

impl InvalidLockComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> DataComponentRemainderFix {
        DataComponentRemainderFix::new(output_schema, "InvalidLockComponentPredicateFix", "minecraft:lock", fix_lock)
    }
}

pub fn fix_lock(input: Dynamic) -> Option<Dynamic> {
    if is_broken_lock(&input) { None } else { Some(input) }
}

fn is_broken_lock(input: &Dynamic) -> bool {
    is_map_with_one_field(input, "components", |components| {
        is_map_with_one_field(components, "minecraft:custom_name", |custom_name| custom_name.as_string().into_result().ok().as_deref() == Some(INVALID_LOCK_CUSTOM_NAME))
    })
}

fn is_map_with_one_field(input: &Dynamic, field_name: &str, predicate: impl FnOnce(&Dynamic) -> bool) -> bool {
    match input.get_map_values().into_result() {
        Ok(map) if map.len() == 1 => input.get(field_name).result().filter(predicate).is_some(),
        _ => false,
    }
}
