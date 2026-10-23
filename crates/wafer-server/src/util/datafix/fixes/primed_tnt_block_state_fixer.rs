use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct PrimedTntBlockStateFixer;

impl PrimedTntBlockStateFixer {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix::new(output_schema, true, "PrimedTnt BlockState fixer", ENTITY, "minecraft:tnt", fix)
    }
}

fn rename_fuse(input: Dynamic) -> Dynamic {
    match input.get("Fuse").result() {
        Some(fuse_value) => input.set("fuse", fuse_value),
        None => input,
    }
}

fn insert_block_state(input: Dynamic) -> Dynamic {
    let block_state = input.create_map([(input.create_string("Name"), input.create_string("minecraft:tnt"))]);
    input.set("block_state", block_state)
}

fn fix(input: Dynamic) -> Dynamic {
    rename_fuse(insert_block_state(input))
}
