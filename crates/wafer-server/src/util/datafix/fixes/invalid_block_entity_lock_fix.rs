use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::invalid_lock_component_fix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct InvalidBlockEntityLockFix {
    base: DataFixBase,
}

impl InvalidBlockEntityLockFix {
    pub fn new(output_schema: &Arc<Schema>) -> InvalidBlockEntityLockFix {
        InvalidBlockEntityLockFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for InvalidBlockEntityLockFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("BlockEntityLockToComponentFix", self.get_input_schema().get_type(BLOCK_ENTITY), |block_entity| {
            block_entity.update_remainder(|remainder| {
                let Some(lock) = remainder.get("lock").result() else { return remainder };
                match invalid_lock_component_fix::fix_lock(lock) {
                    Some(new_lock) => remainder.set("lock", new_lock),
                    None => remainder.remove("lock"),
                }
            })
        })
    }
}
