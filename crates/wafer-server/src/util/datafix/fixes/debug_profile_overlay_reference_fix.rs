use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DEBUG_PROFILE;

pub struct DebugProfileOverlayReferenceFix {
    base: DataFixBase,
}

impl DebugProfileOverlayReferenceFix {
    pub fn new(output_schema: &Arc<Schema>) -> DebugProfileOverlayReferenceFix {
        DebugProfileOverlayReferenceFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for DebugProfileOverlayReferenceFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("DebugProfileOverlayReferenceFix", self.get_input_schema().get_type(DEBUG_PROFILE), |typed| {
            typed.update_remainder(|file| {
                file.update("custom", |custom| {
                    custom.update_map_values(|(key, value)| {
                        let value = if value.as_string_or("") == "inF3" { value.create_string("inOverlay") } else { value };
                        (key, value)
                    })
                })
            })
        })
    }
}
