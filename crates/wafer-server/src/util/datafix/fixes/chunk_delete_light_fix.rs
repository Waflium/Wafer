use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

pub struct ChunkDeleteLightFix {
    base: DataFixBase,
}

impl ChunkDeleteLightFix {
    pub fn new(output_schema: &Arc<Schema>) -> ChunkDeleteLightFix {
        ChunkDeleteLightFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for ChunkDeleteLightFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let sections_finder = find_field(&chunk_type, "sections");
        self.fix_type_everywhere_typed(&format!("ChunkDeleteLightFix for {}", self.get_output_schema().version_key), chunk_type, move |chunk| {
            let chunk = chunk.update_remainder(|tag| tag.remove("isLightOn"));
            chunk.update_typed(&sections_finder, None, |section| section.update_remainder(|tag| tag.remove("BlockLight").remove("SkyLight")))
        })
    }
}
