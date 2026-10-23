use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsMusicToastFix {
    base: DataFixBase,
}

impl OptionsMusicToastFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> OptionsMusicToastFix {
        OptionsMusicToastFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for OptionsMusicToastFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsMusicToastFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| {
                let creator = tag.clone();
                tag.rename_and_fix_field("showNowPlayingToast", "musicToast", |old| {
                    creator.create_string(if old.as_string_or("false") == "false" { "never" } else { "pause_and_toast" })
                })
            })
        })
    }
}
