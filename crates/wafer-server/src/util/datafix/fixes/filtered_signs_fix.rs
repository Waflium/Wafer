use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct FilteredSignsFix;

impl FilteredSignsFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix::new(output_schema, false, "Remove filtered text from signs", BLOCK_ENTITY, "minecraft:sign", fix)
    }
}

fn fix(input: Dynamic) -> Dynamic {
    input.remove("FilteredText1").remove("FilteredText2").remove("FilteredText3").remove("FilteredText4")
}
