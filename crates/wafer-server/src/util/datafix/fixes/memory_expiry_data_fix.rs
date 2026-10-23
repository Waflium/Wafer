use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct MemoryExpiryDataFix;

impl MemoryExpiryDataFix {
    pub fn new(schema: &Arc<Schema>, entity_type: &str) -> NamedEntityFix {
        NamedEntityFix::new(schema, false, &format!("Memory expiry data fix ({entity_type})"), ENTITY, entity_type, |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    input.update("Brain", update_brain)
}

fn update_brain(input: Dynamic) -> Dynamic {
    input.update("memories", update_memories)
}

fn update_memories(memories: Dynamic) -> Dynamic {
    memories.update_map_values(update_memory_entry)
}

fn update_memory_entry(memory_entry: (Dynamic, Dynamic)) -> (Dynamic, Dynamic) {
    (memory_entry.0, wrap_memory_value(memory_entry.1))
}

fn wrap_memory_value(dynamic: Dynamic) -> Dynamic {
    let key = dynamic.create_string("value");
    dynamic.create_map([(key, dynamic.clone())])
}
