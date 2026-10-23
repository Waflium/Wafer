use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub struct ItemStackMapIdFix {
    base: DataFixBase,
}

impl ItemStackMapIdFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemStackMapIdFix {
        ItemStackMapIdFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemStackMapIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let tag_finder = find_field(&item_stack_type, "tag");
        self.fix_type_everywhere_typed("ItemInstanceMapIdFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) == Some("minecraft:filled_map") {
                let rest = input.get_remainder();
                let tag = input.get_or_create_typed(&tag_finder);
                let tag_rest = tag.get_remainder();
                let map = tag_rest.create_int(rest.get("Damage").as_int(0));
                let tag_rest = tag_rest.set("map", map);
                let tag = tag.set_remainder(tag_rest);
                input.set(&tag_finder, tag)
            } else {
                input
            }
        })
    }
}
