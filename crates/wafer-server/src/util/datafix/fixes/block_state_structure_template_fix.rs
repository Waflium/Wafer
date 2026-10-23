use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::references::BLOCK_STATE;

pub struct BlockStateStructureTemplateFix {
    base: DataFixBase,
}

impl BlockStateStructureTemplateFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> BlockStateStructureTemplateFix {
        BlockStateStructureTemplateFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for BlockStateStructureTemplateFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("BlockStateStructureTemplateFix", self.get_input_schema().get_type(BLOCK_STATE), |input| {
            input.update_remainder(block_state_data::upgrade_block_state_tag)
        })
    }
}
