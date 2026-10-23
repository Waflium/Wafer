use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{Dynamic, OptionalDynamic};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::CHUNK;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

const STATUSES_TO_SKIP_BLENDING: [&str; 4] = ["minecraft:empty", "minecraft:structure_starts", "minecraft:structure_references", "minecraft:biomes"];

pub struct BlendingDataFix {
    base: DataFixBase,
    name: String,
}

impl BlendingDataFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlendingDataFix {
        BlendingDataFix { base: DataFixBase::new(output_schema, false), name: format!("Blending Data Fix v{}", output_schema.version_key) }
    }
}

impl DataFix for BlendingDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_output_schema().get_type(CHUNK);
        self.fix_type_everywhere_typed(&self.name, chunk_type, |chunk| {
            chunk.update_remainder(|chunk_tag| {
                let context_tag = chunk_tag.get("__context");
                update_chunk_tag(chunk_tag, context_tag)
            })
        })
    }
}

fn update_chunk_tag(chunk_tag: Dynamic, context_tag: OptionalDynamic) -> Dynamic {
    let mut chunk_tag = chunk_tag.remove("blending_data");
    let is_overworld = context_tag.get("dimension").as_string().into_result().unwrap_or_default() == "minecraft:overworld";
    let status_opt = chunk_tag.get("Status").result();
    if is_overworld && let Some(status_opt) = status_opt {
        let status = ensure_namespaced(&status_opt.as_string_or("empty"));
        let below_zero_retrogen_opt = chunk_tag.get("below_zero_retrogen").result();
        if !STATUSES_TO_SKIP_BLENDING.contains(&status.as_str()) {
            chunk_tag = update_blending_data(chunk_tag, 384, -64);
        } else if let Some(below_zero_retrogen) = below_zero_retrogen_opt {
            let target_status = ensure_namespaced(&below_zero_retrogen.get("target_status").as_string_or("empty"));
            if !STATUSES_TO_SKIP_BLENDING.contains(&target_status.as_str()) {
                chunk_tag = update_blending_data(chunk_tag, 256, 0);
            }
        }
    }
    chunk_tag
}

fn update_blending_data(chunk_tag: Dynamic, height: i32, min_y: i32) -> Dynamic {
    let blending_data = chunk_tag.create_map([
        (chunk_tag.create_string("min_section"), chunk_tag.create_int(min_y >> 4)),
        (chunk_tag.create_string("max_section"), chunk_tag.create_int((min_y + height) >> 4)),
    ]);
    chunk_tag.set("blending_data", blending_data)
}
