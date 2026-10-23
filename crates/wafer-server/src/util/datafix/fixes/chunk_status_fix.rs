use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

pub struct ChunkStatusFix {
    base: DataFixBase,
}

impl ChunkStatusFix {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> ChunkStatusFix {
        ChunkStatusFix { base: DataFixBase::new(schema, changes_type) }
    }
}

impl DataFix for ChunkStatusFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_f = find_field(&chunk_type, "Level");
        self.fix_type_everywhere_typed_to("ChunkStatusFix", chunk_type, self.get_output_schema().get_type(CHUNK), move |input| {
            input.update_typed(&level_f, None, |level| {
                level.update_remainder(|tag| {
                    let status = tag.get("Status").as_string_or("empty");
                    if status == "postprocessed" {
                        let fullchunk = tag.create_string("fullchunk");
                        tag.set("Status", fullchunk)
                    } else {
                        tag
                    }
                })
            })
        })
    }
}
