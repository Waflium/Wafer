use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsSetGraphicsPresetToCustomFix {
    base: DataFixBase,
}

impl OptionsSetGraphicsPresetToCustomFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsSetGraphicsPresetToCustomFix {
        OptionsSetGraphicsPresetToCustomFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for OptionsSetGraphicsPresetToCustomFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("graphicsPreset set to \"custom\"", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| {
                let custom = tag.create_string("custom");
                tag.set("graphicsPreset", custom)
            })
        })
    }
}
