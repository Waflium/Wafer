use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub struct PotDecorationsComponentUnflatteningFix {
    base: DataFixBase,
}

impl PotDecorationsComponentUnflatteningFix {
    pub fn new(output_schema: &Arc<Schema>) -> PotDecorationsComponentUnflatteningFix {
        PotDecorationsComponentUnflatteningFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for PotDecorationsComponentUnflatteningFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.write_fix_and_read(
            "Pot decoration structure fix",
            self.get_input_schema().get_type(DATA_COMPONENTS),
            self.get_output_schema().get_type(DATA_COMPONENTS),
            |components| components.update("minecraft:pot_decorations", unpack_list),
        )
    }
}

pub fn unpack_list(original: Dynamic) -> Dynamic {
    let decoration_ids = original.as_stream_opt();
    if !decoration_ids.is_success() {
        return original;
    }
    let decoration_id_list: Vec<Option<String>> = decoration_ids.value.unwrap_or_default().iter().map(|id| id.as_string().into_result().ok()).collect();
    let mut result = Vec::new();
    for i in 0..4 {
        let decoration_id = match decoration_id_list.get(i) {
            Some(id) => id.clone().unwrap_or_else(|| "minecraft:brick".to_string()),
            None => "minecraft:brick".to_string(),
        };
        if decoration_id.is_empty() {
            return original;
        }
        let side_name = match i {
            0 => "back",
            1 => "left",
            2 => "right",
            _ => "front",
        };
        let new_stack = original.create_map([(original.create_string("id"), original.create_string(decoration_id))]);
        result.push((original.create_string(side_name), new_stack));
    }
    original.create_map(result)
}
