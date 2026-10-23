use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsLowerCaseLanguageFix {
    base: DataFixBase,
}

impl OptionsLowerCaseLanguageFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> OptionsLowerCaseLanguageFix {
        OptionsLowerCaseLanguageFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for OptionsLowerCaseLanguageFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsLowerCaseLanguageFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| match tag.get("lang").as_string().into_result().ok() {
                Some(lang) => {
                    let value = tag.create_string(lang.to_lowercase());
                    tag.set("lang", value)
                }
                None => tag,
            })
        })
    }
}
