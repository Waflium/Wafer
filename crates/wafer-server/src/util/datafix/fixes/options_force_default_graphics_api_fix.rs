use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsForceDefaultGraphicsApiFix {
    base: DataFixBase,
}

impl OptionsForceDefaultGraphicsApiFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsForceDefaultGraphicsApiFix {
        OptionsForceDefaultGraphicsApiFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for OptionsForceDefaultGraphicsApiFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsForceDefaultGraphicsApiFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| {
                let backend = tag.create_string("default");
                tag.set("preferredGraphicsBackend", backend)
            })
        })
    }
}
