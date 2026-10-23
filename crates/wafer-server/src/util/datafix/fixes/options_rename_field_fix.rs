use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsRenameFieldFix {
    base: DataFixBase,
    fix_name: String,
    field_from: String,
    field_to: String,
}

impl OptionsRenameFieldFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool, fix_name: &str, field_from: &str, field_to: &str) -> OptionsRenameFieldFix {
        OptionsRenameFieldFix { base: DataFixBase::new(output_schema, changes_type), fix_name: fix_name.to_string(), field_from: field_from.to_string(), field_to: field_to.to_string() }
    }
}

impl DataFix for OptionsRenameFieldFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let (field_from, field_to) = (self.field_from.clone(), self.field_to.clone());
        self.fix_type_everywhere_typed(&self.fix_name, self.get_input_schema().get_type(OPTIONS), move |input| {
            input.update_remainder(|tag| tag.rename_field(&field_from, &field_to))
        })
    }
}
