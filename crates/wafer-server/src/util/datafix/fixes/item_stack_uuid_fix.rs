use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::util::datafix::fixes::abstract_uuid_fix::{AbstractUUIDFix, replace_uuid_least_most, replace_uuid_string};
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub struct ItemStackUUIDFix {
    abstract_uuid_fix: AbstractUUIDFix,
}

impl ItemStackUUIDFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemStackUUIDFix {
        ItemStackUUIDFix { abstract_uuid_fix: AbstractUUIDFix::new(output_schema, ITEM_STACK) }
    }
}

impl DataFix for ItemStackUUIDFix {
    fn base(&self) -> &DataFixBase {
        &self.abstract_uuid_fix.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let item_stack_type = self.get_input_schema().get_type(self.abstract_uuid_fix.type_reference);
        let item_tag_finder = find_field(&item_stack_type, "tag");
        self.fix_type_everywhere_typed("ItemStackUUIDFix", item_stack_type, move |input| {
            let is_player_head = input.get_optional(&id_finder).is_some_and(|id| id.as_str() == Some("minecraft:player_head"));
            input.update_typed(&item_tag_finder, None, |typed_tag| {
                typed_tag.update_remainder(|tag| {
                    let tag = update_attribute_modifiers(tag);
                    if is_player_head { update_skull_owner(tag) } else { tag }
                })
            })
        })
    }
}

fn update_attribute_modifiers(tag: Dynamic) -> Dynamic {
    tag.update("AttributeModifiers", |modifiers| {
        modifiers.create_list(modifiers.as_stream().into_iter().map(|modifier| replace_uuid_least_most(&modifier, "UUID", "UUID").unwrap_or(modifier)))
    })
}

fn update_skull_owner(tag: Dynamic) -> Dynamic {
    tag.update("SkullOwner", |skull_owner| replace_uuid_string(&skull_owner, "Id", "Id").unwrap_or(skull_owner))
}
