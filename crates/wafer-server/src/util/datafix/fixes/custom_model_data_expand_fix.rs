use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{Dynamic, Number};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub struct CustomModelDataExpandFix {
    base: DataFixBase,
}

impl CustomModelDataExpandFix {
    pub fn new(output_schema: &Arc<Schema>) -> CustomModelDataExpandFix {
        CustomModelDataExpandFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for CustomModelDataExpandFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("Custom Model Data expansion", self.get_input_schema().get_type(DATA_COMPONENTS), |component| {
            component.update_remainder(|tag| tag.update("minecraft:custom_model_data", expand))
        })
    }
}

fn expand(cmd: Dynamic) -> Dynamic {
    let current_value = cmd.as_number_or(Number::Float(0.0)).float_value();
    cmd.create_map([(cmd.create_string("floats"), cmd.create_list([cmd.create_float(current_value)]))])
}
