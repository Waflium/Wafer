use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

const DECORATED_POT_ID: &str = "minecraft:decorated_pot";

pub struct DecoratedPotFieldRenameFix {
    base: DataFixBase,
}

impl DecoratedPotFieldRenameFix {
    pub fn new(output_schema: &Arc<Schema>) -> DecoratedPotFieldRenameFix {
        DecoratedPotFieldRenameFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for DecoratedPotFieldRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_decorated_pot = self.get_input_schema().get_choice_type(BLOCK_ENTITY, DECORATED_POT_ID);
        let new_decorated_pot = self.get_output_schema().get_choice_type(BLOCK_ENTITY, DECORATED_POT_ID);
        self.convert_unchecked("DecoratedPotFieldRenameFix", old_decorated_pot, new_decorated_pot)
    }
}
