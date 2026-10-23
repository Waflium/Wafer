use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::named_choice;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct SignsOpFlagFix {
    base: DataFixBase,
}

impl SignsOpFlagFix {
    pub fn new(output_schema: &Arc<Schema>) -> SignsOpFlagFix {
        SignsOpFlagFix { base: DataFixBase::new(output_schema, false) }
    }

    fn make_rule_for_type(&self, entity_name: &str) -> TypeRewriteRule {
        let entity_type = self.get_input_schema().get_choice_type(BLOCK_ENTITY, entity_name);
        let entity_finder = named_choice(entity_name, entity_type.clone());
        self.fix_type_everywhere_typed(&format!("Add allow-op flag to {entity_name}"), self.get_input_schema().get_type(BLOCK_ENTITY), move |input| {
            input.update_typed(&entity_finder, Some(&entity_type), |entity| {
                entity.update_remainder(|remainder| {
                    let allow = remainder.create_boolean(true);
                    remainder.set("allow_op_features", allow)
                })
            })
        })
    }
}

impl DataFix for SignsOpFlagFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        TypeRewriteRule::seq(vec![self.make_rule_for_type("minecraft:sign"), self.make_rule_for_type("minecraft:hanging_sign")])
    }
}
