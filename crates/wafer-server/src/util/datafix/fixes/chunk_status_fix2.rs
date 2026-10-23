use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

const RENAMES_AND_DOWNGRADES: [(&str, &str); 10] = [
    ("structure_references", "empty"),
    ("biomes", "empty"),
    ("base", "surface"),
    ("carved", "carvers"),
    ("liquid_carved", "liquid_carvers"),
    ("decorated", "features"),
    ("lighted", "light"),
    ("mobs_spawned", "spawn"),
    ("finalized", "heightmaps"),
    ("fullchunk", "full"),
];

pub struct ChunkStatusFix2 {
    base: DataFixBase,
}

impl ChunkStatusFix2 {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> ChunkStatusFix2 {
        ChunkStatusFix2 { base: DataFixBase::new(schema, changes_type) }
    }
}

impl DataFix for ChunkStatusFix2 {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_f = find_field(&chunk_type, "Level");
        self.fix_type_everywhere_typed_to("ChunkStatusFix2", chunk_type, self.get_output_schema().get_type(CHUNK), move |input| {
            input.update_typed(&level_f, None, |level| {
                let tag = level.get_remainder();
                let status = tag.get("Status").as_string_or("empty");
                let new_status = RENAMES_AND_DOWNGRADES.iter().find(|(from, _)| *from == status).map_or("empty", |(_, to)| to);
                if status == new_status {
                    level
                } else {
                    let new_status = tag.create_string(new_status);
                    level.set_remainder(tag.set("Status", new_status))
                }
            })
        })
    }
}
