use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{Dynamic, OptionalDynamic};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::CHUNK;

pub struct BlendingDataRemoveFromNetherEndFix {
    base: DataFixBase,
}

impl BlendingDataRemoveFromNetherEndFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlendingDataRemoveFromNetherEndFix {
        BlendingDataRemoveFromNetherEndFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for BlendingDataRemoveFromNetherEndFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_output_schema().get_type(CHUNK);
        self.fix_type_everywhere_typed("BlendingDataRemoveFromNetherEndFix", chunk_type, |chunk| {
            chunk.update_remainder(|chunk_tag| {
                let context_tag = chunk_tag.get("__context");
                update_chunk_tag(chunk_tag, context_tag)
            })
        })
    }
}

fn update_chunk_tag(chunk_tag: Dynamic, context_tag: OptionalDynamic) -> Dynamic {
    let is_overworld = context_tag.get("dimension").as_string_or("") == "minecraft:overworld";
    if is_overworld { chunk_tag } else { chunk_tag.remove("blending_data") }
}
