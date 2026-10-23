use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::field_finder;
use crate::datafixers::types::const_type;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;
use crate::util::datafix::legacy_component_data_fix_utils;

const NAMEABLE_BLOCK_ENTITIES: [&str; 11] = [
    "minecraft:beacon",
    "minecraft:banner",
    "minecraft:brewing_stand",
    "minecraft:chest",
    "minecraft:trapped_chest",
    "minecraft:dispenser",
    "minecraft:dropper",
    "minecraft:enchanting_table",
    "minecraft:furnace",
    "minecraft:hopper",
    "minecraft:shulker_box",
];

pub struct BlockEntityCustomNameToComponentFix {
    base: DataFixBase,
}

impl BlockEntityCustomNameToComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlockEntityCustomNameToComponentFix {
        BlockEntityCustomNameToComponentFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for BlockEntityCustomNameToComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let id_finder = field_finder("id", const_type(ConstType::NamespacedString));
        let input_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let output_type = self.get_output_schema().get_type(BLOCK_ENTITY);
        let patched_input_type = extra_data_fix_utils::patch_sub_type(&input_type, &input_type, &output_type);
        let target_type = output_type.clone();
        self.fix_type_everywhere_typed_to("BlockEntityCustomNameToComponentFix", input_type, output_type, move |input| {
            let id = input.get_optional(&id_finder).and_then(|id| id.as_str().map(str::to_string));
            match id {
                Some(id) if !NAMEABLE_BLOCK_ENTITIES.contains(&id.as_str()) => extra_data_fix_utils::cast(&target_type, input),
                _ => crate::util::write_and_read_typed_or_throw(&extra_data_fix_utils::cast(&patched_input_type, input), &target_type, fix_tag_custom_name),
            }
        })
    }
}

pub fn fix_tag_custom_name(tag: Dynamic) -> Dynamic {
    let name = tag.get("CustomName").as_string_or("");
    if name.is_empty() {
        tag.remove("CustomName")
    } else {
        let component = legacy_component_data_fix_utils::create_plain_text_component(&name);
        tag.set("CustomName", component)
    }
}
