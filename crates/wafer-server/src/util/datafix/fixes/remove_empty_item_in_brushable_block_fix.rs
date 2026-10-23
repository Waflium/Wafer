use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct RemoveEmptyItemInBrushableBlockFix;

impl RemoveEmptyItemInBrushableBlockFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix::new(output_schema, false, "RemoveEmptyItemInSuspiciousBlockFix", BLOCK_ENTITY, "minecraft:brushable_block", fix)
    }
}

fn fix(input: Dynamic) -> Dynamic {
    match input.get("item").result() {
        Some(item) if is_empty_stack(&item) => input.remove("item"),
        _ => input,
    }
}

fn is_empty_stack(item: &Dynamic) -> bool {
    let id = ensure_namespaced(&item.get("id").as_string_or("minecraft:air"));
    let count = item.get("count").as_int(0);
    id == "minecraft:air" || count == 0
}
