use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::util::datafix::fixes::references::{ITEM_STACK, TEXT_COMPONENT};
use crate::util::datafix::legacy_component_data_fix_utils;

pub struct ItemCustomNameToComponentFix {
    base: DataFixBase,
}

impl ItemCustomNameToComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemCustomNameToComponentFix {
        ItemCustomNameToComponentFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for ItemCustomNameToComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let text_component_type = self.get_input_schema().get_type(TEXT_COMPONENT);
        let tag_finder = find_field(&item_stack_type, "tag");
        let display_finder = find_field(&tag_finder.ty(), "display");
        let custom_name_finder = find_field(&display_finder.ty(), "Name");
        let text_component_finder = type_finder(text_component_type);
        self.fix_type_everywhere_typed("ItemCustomNameToComponentFix", item_stack_type, move |item_stack| {
            item_stack.update_typed(&tag_finder, None, |tag| {
                tag.update_typed(&display_finder, None, |display| {
                    display.update_typed(&custom_name_finder, None, |custom_name| {
                        custom_name.update(&text_component_finder, |text_component| text_component.map_string(legacy_component_data_fix_utils::create_text_component_json))
                    })
                })
            })
        })
    }
}
