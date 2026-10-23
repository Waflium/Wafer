use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsFancyGraphicsToGraphicsModeFix {
    base: DataFixBase,
}

impl OptionsFancyGraphicsToGraphicsModeFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsFancyGraphicsToGraphicsModeFix {
        OptionsFancyGraphicsToGraphicsModeFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for OptionsFancyGraphicsToGraphicsModeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("fancyGraphics to graphicsMode", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| tag.rename_and_fix_field("fancyGraphics", "graphicsMode", fix_graphics_mode))
        })
    }
}

fn fix_graphics_mode(field: Dynamic) -> Dynamic {
    if field.as_string_or("true") == "true" { field.create_string("1") } else { field.create_string("0") }
}
