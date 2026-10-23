use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub struct RemoveBlockTransformerComponentFix {
    base: DataFixBase,
}

impl RemoveBlockTransformerComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> RemoveBlockTransformerComponentFix {
        RemoveBlockTransformerComponentFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for RemoveBlockTransformerComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("Remove inline block transformer component", self.get_input_schema().get_type(DATA_COMPONENTS), |components| {
            components.update_remainder(|remainder| remainder.remove("minecraft:block_transformer"))
        })
    }
}
