use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

pub struct ChunkBiomeFix {
    base: DataFixBase,
}

impl ChunkBiomeFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ChunkBiomeFix {
        ChunkBiomeFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ChunkBiomeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_finder = find_field(&chunk_type, "Level");
        self.fix_type_everywhere_typed("Leaves fix", chunk_type, move |chunk| chunk.update_typed(&level_finder, None, |level| level.update_remainder(fix_tag)))
    }
}

fn fix_tag(tag: Dynamic) -> Dynamic {
    let Some(old_biomes) = tag.get("Biomes").result().and_then(|biomes| biomes.as_int_stream_opt().into_result().ok()) else { return tag };
    if old_biomes.len() != 256 {
        return tag;
    }
    let mut new_biomes = [0i32; 1024];
    for z in 0..4 {
        for x in 0..4 {
            let old_x = (x << 2) + 2;
            let old_z = (z << 2) + 2;
            let index = old_z << 4 | old_x;
            new_biomes[z << 2 | x] = old_biomes[index];
        }
    }
    for y_slice in 1..64 {
        new_biomes.copy_within(0..16, y_slice * 16);
    }
    let biomes = tag.create_int_list(new_biomes);
    tag.set("Biomes", biomes)
}
