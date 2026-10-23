use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::field_finder;
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub struct BedItemColorFix {
    base: DataFixBase,
}

impl BedItemColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> BedItemColorFix {
        BedItemColorFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for BedItemColorFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        self.fix_type_everywhere_typed("BedItemColorFix", self.get_input_schema().get_type(ITEM_STACK), move |input| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) == Some("minecraft:bed") {
                let tag = input.get_remainder();
                if tag.get("Damage").as_int(0) == 0 {
                    let damage = tag.create_short(14);
                    return input.set_remainder(tag.set("Damage", damage));
                }
            }
            input
        })
    }
}
