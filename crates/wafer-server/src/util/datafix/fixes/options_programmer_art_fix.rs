use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsProgrammerArtFix {
    base: DataFixBase,
}

impl OptionsProgrammerArtFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsProgrammerArtFix {
        OptionsProgrammerArtFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for OptionsProgrammerArtFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsProgrammerArtFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| tag.update("resourcePacks", fix_list).update("incompatibleResourcePacks", fix_list))
        })
    }
}

fn fix_list(entry: Dynamic) -> Dynamic {
    match entry.as_string().into_result() {
        Ok(string) => entry.create_string(string.replace("\"programer_art\"", "\"programmer_art\"")),
        Err(_) => entry,
    }
}
