use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityHealthFix {
    base: DataFixBase,
}

impl EntityHealthFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityHealthFix {
        EntityHealthFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    let old_heal_f = input.get("HealF").as_number().into_result();
    let old_health = input.get("Health").as_number().into_result();
    let (input, health) = match (old_heal_f, old_health) {
        (Ok(heal_f), _) => (input.remove("HealF"), heal_f.float_value()),
        (Err(_), Ok(health)) => (input, health.float_value()),
        (Err(_), Err(_)) => return input,
    };
    let health = input.create_float(health);
    input.set("Health", health)
}

impl DataFix for EntityHealthFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("EntityHealthFix", self.get_input_schema().get_type(ENTITY), |input| input.update_remainder(fix_tag))
    }
}
