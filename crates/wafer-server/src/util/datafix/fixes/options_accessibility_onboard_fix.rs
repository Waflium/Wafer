use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsAccessibilityOnboardFix {
    base: DataFixBase,
}

impl OptionsAccessibilityOnboardFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsAccessibilityOnboardFix {
        OptionsAccessibilityOnboardFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for OptionsAccessibilityOnboardFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsAccessibilityOnboardFix", self.get_input_schema().get_type(OPTIONS), |typed| {
            typed.update_remainder(|value| {
                let onboard_accessibility = value.create_string("false");
                value.set("onboardAccessibility", onboard_accessibility)
            })
        })
    }
}
