use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsAmbientOcclusionFix {
    base: DataFixBase,
}

impl OptionsAmbientOcclusionFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsAmbientOcclusionFix {
        OptionsAmbientOcclusionFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for OptionsAmbientOcclusionFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsAmbientOcclusionFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| match tag.get("ao").as_string().into_result() {
                Ok(value) => {
                    let updated = tag.create_string(update_value(&value));
                    tag.set("ao", updated)
                }
                Err(_) => tag,
            })
        })
    }
}

fn update_value(value: &str) -> &str {
    match value {
        "0" => "false",
        "1" | "2" => "true",
        _ => value,
    }
}
