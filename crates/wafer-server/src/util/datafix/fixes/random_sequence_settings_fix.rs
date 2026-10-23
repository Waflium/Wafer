use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::SAVED_DATA_RANDOM_SEQUENCES;

pub struct RandomSequenceSettingsFix {
    base: DataFixBase,
}

impl RandomSequenceSettingsFix {
    pub fn new(output_schema: &Arc<Schema>) -> RandomSequenceSettingsFix {
        RandomSequenceSettingsFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for RandomSequenceSettingsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("RandomSequenceSettingsFix", self.get_input_schema().get_type(SAVED_DATA_RANDOM_SEQUENCES), |input| {
            input.update_remainder(|container| container.update("data", |tag| tag.empty_map().set("sequences", tag)))
        })
    }
}
