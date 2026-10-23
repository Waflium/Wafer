use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

const NEW_TRANSLATION: &str = "item.minecraft.buried_treasure_map";
const OLD_TRANSLATION: &str = "filled_map.buried_treasure";

pub struct UnfilledBuriedTreasureMapFix {
    base: DataFixBase,
}

impl UnfilledBuriedTreasureMapFix {
    pub fn new(output_schema: &Arc<Schema>) -> UnfilledBuriedTreasureMapFix {
        UnfilledBuriedTreasureMapFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for UnfilledBuriedTreasureMapFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let components_finder = find_field(&item_stack_type, "components");
        self.fix_type_everywhere_typed("UnfilledBuriedTreasureMapFix", item_stack_type, move |item_stack| {
            let id = item_stack.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) == Some("minecraft:map") { item_stack.update_typed(&components_finder, None, fix) } else { item_stack }
        })
    }
}

fn fix(typed: Typed) -> Typed {
    let ty = typed.ty.clone();
    crate::util::write_and_read_typed_or_throw(&typed, &ty, |value| {
        let holds_legacy_name = value
            .get("minecraft:item_name")
            .result()
            .and_then(|name| extra_data_fix_utils::get_plain_translation_key(&name))
            .is_some_and(|name_key| name_key == OLD_TRANSLATION);
        if holds_legacy_name {
            value.update("minecraft:item_name", |item_name| {
                let translation = item_name.create_string(NEW_TRANSLATION);
                item_name.set("translate", translation)
            })
        } else {
            value
        }
    })
}
