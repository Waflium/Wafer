use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DEBUG_PROFILE;

pub struct DebugProfileRenameEntryFix {
    base: DataFixBase,
    old_name: String,
    new_name: String,
}

impl DebugProfileRenameEntryFix {
    pub fn new(output_schema: &Arc<Schema>, old_name: &str, new_name: &str) -> DebugProfileRenameEntryFix {
        DebugProfileRenameEntryFix { base: DataFixBase::new(output_schema, false), old_name: old_name.to_string(), new_name: new_name.to_string() }
    }
}

impl DataFix for DebugProfileRenameEntryFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let (old_name, new_name) = (self.old_name.clone(), self.new_name.clone());
        self.fix_type_everywhere_typed("DebugProfilePostEffectsFix", self.get_input_schema().get_type(DEBUG_PROFILE), move |input| {
            input.update_remainder(|remainder| remainder.update("custom", |custom| custom.rename_field(&old_name, &new_name)))
        })
    }
}
