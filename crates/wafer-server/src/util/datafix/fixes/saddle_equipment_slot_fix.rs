use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::type_finder;
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::ENTITY;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

const ENTITIES_WITH_SADDLE_ITEM: [&str; 8] = [
    "minecraft:horse",
    "minecraft:skeleton_horse",
    "minecraft:zombie_horse",
    "minecraft:donkey",
    "minecraft:mule",
    "minecraft:camel",
    "minecraft:llama",
    "minecraft:trader_llama",
];
const ENTITIES_WITH_SADDLE_FLAG: [&str; 2] = ["minecraft:pig", "minecraft:strider"];
const SADDLE_FLAG: &str = "Saddle";
const NEW_SADDLE: &str = "saddle";

pub struct SaddleEquipmentSlotFix {
    base: DataFixBase,
}

impl SaddleEquipmentSlotFix {
    pub fn new(output_schema: &Arc<Schema>) -> SaddleEquipmentSlotFix {
        SaddleEquipmentSlotFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for SaddleEquipmentSlotFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let entity_id_type = self.get_input_schema().find_choice_type(ENTITY);
        let entity_id_f = type_finder(entity_id_type);
        let input_type = self.get_input_schema().get_type(ENTITY);
        let output_type = self.get_output_schema().get_type(ENTITY);
        let patched_input_type = extra_data_fix_utils::patch_sub_type(&input_type, &input_type, &output_type);
        let output = output_type.clone();
        self.fix_type_everywhere_typed_to("SaddleEquipmentSlotFix", input_type, output_type, move |input| {
            let entity_id = match input.get_optional(&entity_id_f) {
                Some(Value::Choice(key, _)) => ensure_namespaced(&key),
                _ => String::new(),
            };
            let fixed_input = extra_data_fix_utils::cast(&patched_input_type, input.clone());
            if ENTITIES_WITH_SADDLE_ITEM.contains(&entity_id.as_str()) {
                crate::util::write_and_read_typed_or_throw(&fixed_input, &output, fix_entity_with_saddle_item)
            } else if ENTITIES_WITH_SADDLE_FLAG.contains(&entity_id.as_str()) {
                crate::util::write_and_read_typed_or_throw(&fixed_input, &output, fix_entity_with_saddle_flag)
            } else {
                extra_data_fix_utils::cast(&output, input)
            }
        })
    }
}

fn fix_entity_with_saddle_item(input: Dynamic) -> Dynamic {
    if input.get("SaddleItem").result().is_none() { input } else { fix_drop_chances(input.rename_field("SaddleItem", NEW_SADDLE)) }
}

fn fix_entity_with_saddle_flag(tag: Dynamic) -> Dynamic {
    let has_saddle = tag.get(SADDLE_FLAG).as_boolean_or(false);
    let tag = tag.remove(SADDLE_FLAG);
    if !has_saddle {
        return tag;
    }
    let saddle_item = tag.empty_map().set("id", tag.create_string("minecraft:saddle")).set("count", tag.create_int(1));
    fix_drop_chances(tag.set(NEW_SADDLE, saddle_item))
}

fn fix_drop_chances(tag: Dynamic) -> Dynamic {
    let drop_chances = tag.get("drop_chances").or_else_empty_map().set(NEW_SADDLE, tag.create_float(2.0));
    tag.set("drop_chances", drop_chances)
}
