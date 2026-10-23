use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};

pub static NAMES_BY_COLOR: [&str; 16] = [
    "minecraft:white_shulker_box",
    "minecraft:orange_shulker_box",
    "minecraft:magenta_shulker_box",
    "minecraft:light_blue_shulker_box",
    "minecraft:yellow_shulker_box",
    "minecraft:lime_shulker_box",
    "minecraft:pink_shulker_box",
    "minecraft:gray_shulker_box",
    "minecraft:silver_shulker_box",
    "minecraft:cyan_shulker_box",
    "minecraft:purple_shulker_box",
    "minecraft:blue_shulker_box",
    "minecraft:brown_shulker_box",
    "minecraft:green_shulker_box",
    "minecraft:red_shulker_box",
    "minecraft:black_shulker_box",
];

pub struct ItemShulkerBoxColorFix {
    base: DataFixBase,
}

impl ItemShulkerBoxColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ItemShulkerBoxColorFix {
        ItemShulkerBoxColorFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ItemShulkerBoxColorFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let tag_finder = find_field(&item_stack_type, "tag");
        let block_entity_finder = find_field(&tag_finder.ty(), "BlockEntityTag");
        self.fix_type_everywhere_typed("ItemShulkerBoxColorFix", item_stack_type, move |input| {
            let id = input.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) == Some("minecraft:shulker_box") {
                if let Some(tag) = input.get_optional_typed(&tag_finder) {
                    if let Some(block_entity) = tag.get_optional_typed(&block_entity_finder) {
                        let block_entity_rest = block_entity.get_remainder();
                        let color = block_entity_rest.get("Color").as_int(0);
                        let tag = tag.set(&block_entity_finder, block_entity.set_remainder(block_entity_rest));
                        let name = NAMES_BY_COLOR[(color % 16) as usize];
                        return input.set(&tag_finder, tag).update(&id_finder, |_| Value::Prim(Nbt::String(name.to_string())));
                    }
                }
            }
            input
        })
    }
}
