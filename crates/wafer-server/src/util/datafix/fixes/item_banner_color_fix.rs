use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field, remainder_finder};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub struct ItemBannerColorFix {
    base: DataFixBase,
}

impl ItemBannerColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemBannerColorFix {
        ItemBannerColorFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemBannerColorFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let tag_finder = find_field(&item_stack_type, "tag");
        let block_entity_finder = find_field(&tag_finder.ty(), "BlockEntityTag");
        self.fix_type_everywhere_typed("ItemBannerColorFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) != Some("minecraft:banner") {
                return input;
            }
            let mut rest = input.get_remainder();
            if let Some(tag) = input.get_optional_typed(&tag_finder) {
                if let Some(block_entity) = tag.get_optional_typed(&block_entity_finder) {
                    let tag_rest = tag.get_remainder();
                    let block_entity_rest = match block_entity.get_or_create_typed(&remainder_finder()).value {
                        Value::Dynamic(rest) => rest,
                        _ => Dynamic::empty(),
                    };
                    if block_entity_rest.get("Base").as_number().is_success() {
                        let damage = rest.create_short((block_entity_rest.get("Base").as_int(0) & 15) as i16);
                        rest = rest.set("Damage", damage);
                        if let Some(display) = tag_rest.get("display").result() {
                            let pick_marker = display.create_map([(display.create_string("Lore"), display.create_list([display.create_string("(+NBT")]))]);
                            if display == pick_marker {
                                return input.set_remainder(rest);
                            }
                        }
                        let tag = tag.set(&block_entity_finder, block_entity.set_remainder(block_entity_rest));
                        return input.set_remainder(rest).set(&tag_finder, tag);
                    }
                }
            }
            input.set_remainder(rest)
        })
    }
}
