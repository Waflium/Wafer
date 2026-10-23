use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::CHUNK;

pub struct BlendingDataNoValueHeightFix {
    base: DataFixBase,
}

impl BlendingDataNoValueHeightFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlendingDataNoValueHeightFix {
        BlendingDataNoValueHeightFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for BlendingDataNoValueHeightFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("BlendingDataNoValueHeightFix", self.get_input_schema().get_type(CHUNK), |chunk| chunk.update_remainder(fix))
    }
}

fn fix(chunk: Dynamic) -> Dynamic {
    chunk.update("blending_data", |blending_data| {
        blending_data.update("heights", |heights| {
            let values = heights.as_stream().into_iter().map(fix_height_value);
            heights.create_list(values)
        })
    })
}

fn fix_height_value(height: Dynamic) -> Dynamic {
    let height_value = height.as_double(f64::MAX);
    if height_value == f64::MAX { height.create_float(f32::MAX) } else { height.create_float(height_value as f32) }
}
