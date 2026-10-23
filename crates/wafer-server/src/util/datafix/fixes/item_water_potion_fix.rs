use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub struct ItemWaterPotionFix {
    base: DataFixBase,
}

impl ItemWaterPotionFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemWaterPotionFix {
        ItemWaterPotionFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemWaterPotionFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let tag_finder = find_field(&item_stack_type, "tag");
        self.fix_type_everywhere_typed("ItemWaterPotionFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder);
            if let Some("minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion" | "minecraft:tipped_arrow") = id.as_ref().and_then(Value::as_str) {
                let tag = input.get_or_create_typed(&tag_finder);
                let mut tag_rest = tag.get_remainder();
                if !tag_rest.get("Potion").as_string().is_success() {
                    let potion = tag_rest.create_string("minecraft:water");
                    tag_rest = tag_rest.set("Potion", potion);
                }
                return input.set(&tag_finder, tag.set_remainder(tag_rest));
            }
            input
        })
    }
}
