use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::BLOCK_STATE;

pub struct BlockStateFieldNamesFix {
    base: DataFixBase,
}

impl BlockStateFieldNamesFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlockStateFieldNamesFix {
        BlockStateFieldNamesFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for BlockStateFieldNamesFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("BlockStateFieldNamesFix", self.get_input_schema().get_type(BLOCK_STATE), |input| {
            input.update_remainder(|remainder| remainder.rename_field("Name", "id").rename_field("Properties", "properties"))
        })
    }
}
