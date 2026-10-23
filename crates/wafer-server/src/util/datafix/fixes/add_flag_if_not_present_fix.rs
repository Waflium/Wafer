use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;

pub struct AddFlagIfNotPresentFix {
    base: DataFixBase,
    name: String,
    flag_value: bool,
    flag_key: String,
    type_reference: TypeReference,
}

impl AddFlagIfNotPresentFix {
    pub fn new(output_schema: &Arc<Schema>, type_reference: TypeReference, flag_key: &str, flag_value: bool) -> AddFlagIfNotPresentFix {
        AddFlagIfNotPresentFix {
            base: DataFixBase::new(output_schema, true),
            name: format!("AddFlagIfNotPresentFix_{flag_key}={flag_value} for {}", output_schema.version_key),
            flag_value,
            flag_key: flag_key.to_string(),
            type_reference,
        }
    }
}

impl DataFix for AddFlagIfNotPresentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let world_gen_settings_type = self.get_input_schema().get_type(self.type_reference);
        let (flag_key, flag_value) = (self.flag_key.clone(), self.flag_value);
        self.fix_type_everywhere_typed(&self.name, world_gen_settings_type, move |settings| {
            settings.update_remainder(|tag| {
                let flag = tag.get(&flag_key).result().unwrap_or_else(|| tag.create_boolean(flag_value));
                tag.set(&flag_key, flag)
            })
        })
    }
}
