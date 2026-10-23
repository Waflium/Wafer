use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsGraphicsModeSplitFix {
    base: DataFixBase,
    new_field_name: String,
    value_if_fast: String,
    value_if_fancy: String,
    value_if_fabulous: String,
}

impl OptionsGraphicsModeSplitFix {
    pub fn new(output_schema: &Arc<Schema>, new_field_name: &str, value_if_fast: &str, value_if_fancy: &str, value_if_fabulous: &str) -> OptionsGraphicsModeSplitFix {
        OptionsGraphicsModeSplitFix {
            base: DataFixBase::new(output_schema, true),
            new_field_name: new_field_name.to_string(),
            value_if_fast: value_if_fast.to_string(),
            value_if_fancy: value_if_fancy.to_string(),
            value_if_fabulous: value_if_fabulous.to_string(),
        }
    }
}

impl DataFix for OptionsGraphicsModeSplitFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let (new_field_name, fast, fancy, fabulous) = (self.new_field_name.clone(), self.value_if_fast.clone(), self.value_if_fancy.clone(), self.value_if_fabulous.clone());
        self.fix_type_everywhere_typed(&format!("graphicsMode split to {}", self.new_field_name), self.get_input_schema().get_type(OPTIONS), move |input| {
            input.update_remainder(|tag| {
                let value = match tag.get("graphicsMode").as_string().into_result() {
                    Ok(mode) => match mode.as_str() {
                        "2" => fabulous.clone(),
                        "0" => fast.clone(),
                        _ => fancy.clone(),
                    },
                    Err(_) => fancy.clone(),
                };
                let value = tag.create_string(value);
                tag.set(&new_field_name, value)
            })
        })
    }
}
