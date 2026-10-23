use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::lock_component_predicate_fix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct ContainerBlockEntityLockPredicateFix {
    base: DataFixBase,
}

impl ContainerBlockEntityLockPredicateFix {
    pub fn new(output_schema: &Arc<Schema>) -> ContainerBlockEntityLockPredicateFix {
        ContainerBlockEntityLockPredicateFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for ContainerBlockEntityLockPredicateFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("ContainerBlockEntityLockPredicateFix", self.get_input_schema().find_choice_type(BLOCK_ENTITY), fix_block_entity)
    }
}

fn fix_block_entity(entity: Typed) -> Typed {
    entity.update_remainder(|tag| {
        let lock = tag.get("Lock").result().and_then(lock_component_predicate_fix::fix_lock);
        tag.remove("Lock").set_field_if_present("lock", lock)
    })
}
