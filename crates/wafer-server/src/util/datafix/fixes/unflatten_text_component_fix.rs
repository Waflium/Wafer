use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::json_ops;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::TEXT_COMPONENT;
use crate::util::gson;

pub struct UnflattenTextComponentFix {
    base: DataFixBase,
}

impl UnflattenTextComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> UnflattenTextComponentFix {
        UnflattenTextComponentFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for UnflattenTextComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let text_component_type = self.get_input_schema().get_type(TEXT_COMPONENT);
        let new_text_component_type = self.get_output_schema().get_type(TEXT_COMPONENT);
        let output_type = new_text_component_type.clone();
        self.fix_type_everywhere_to("UnflattenTextComponentFix", text_component_type, new_text_component_type, move |input| {
            let Value::Prim(Nbt::String(json_string)) = input else { panic!("Unexpected text component value") };
            crate::util::read_typed_or_throw(&output_type, unflatten_json(&json_string), true).value
        })
    }
}

fn unflatten_json(json_string: &str) -> Dynamic {
    match gson::parse_lenient(json_string) {
        Ok(json) => {
            if !json.is_json_null() {
                match json_ops::convert_element_to_nbt(&json) {
                    Ok(value) => return Dynamic::of(value),
                    Err(message) => error!("Failed to unflatten text component json: {json_string}\n{message}"),
                }
            }
        }
        Err(error) => error!("Failed to unflatten text component json: {json_string}\n{error}"),
    }
    Dynamic::new(Nbt::String(json_string.to_string()))
}
