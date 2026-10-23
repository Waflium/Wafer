use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

pub struct ChunkDeleteIgnoredLightDataFix {
    base: DataFixBase,
}

impl ChunkDeleteIgnoredLightDataFix {
    pub fn new(output_schema: &Arc<Schema>) -> ChunkDeleteIgnoredLightDataFix {
        ChunkDeleteIgnoredLightDataFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for ChunkDeleteIgnoredLightDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let sections_finder = find_field(&chunk_type, "sections");
        self.fix_type_everywhere_typed("ChunkDeleteIgnoredLightDataFix", chunk_type, move |chunk| {
            let is_light_on = chunk.get_remainder().get("isLightOn").as_boolean_or(false);
            if !is_light_on {
                chunk.update_typed(&sections_finder, None, |section| section.update_remainder(|tag| tag.remove("BlockLight").remove("SkyLight")))
            } else {
                chunk
            }
        })
    }
}
