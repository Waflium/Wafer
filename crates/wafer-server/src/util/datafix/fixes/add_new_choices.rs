use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::value::Value;

pub struct AddNewChoices {
    base: DataFixBase,
    name: String,
    ty: TypeReference,
}

impl AddNewChoices {
    pub fn new(output_schema: &Arc<Schema>, name: &str, ty: TypeReference) -> AddNewChoices {
        AddNewChoices { base: DataFixBase::new(output_schema, true), name: name.to_string(), ty }
    }
}

impl DataFix for AddNewChoices {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_type = self.get_input_schema().find_choice_type(self.ty);
        let output_type = self.get_output_schema().find_choice_type(self.ty);
        if input_type.choices().key_type != output_type.choices().key_type {
            panic!("Could not inject: key type is not the same");
        }
        let name = self.name.clone();
        let type_name = self.ty.type_name();
        let output_choice_type = output_type.clone();
        self.fix_type_everywhere_to(&self.name, input_type, output_type, move |input| {
            if let Value::Choice(key, _) = &input
                && output_choice_type.choices().get(key).is_none()
            {
                panic!("{name}: Unknown type {key} in '{type_name}'");
            }
            input
        })
    }
}
