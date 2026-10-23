use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field, remainder_finder};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

const ITEM_NAME_KEY: &str = "minecraft:item_name";
const DECORATION_KEY: &str = "+";

fn decoration_type_to_item_id(decoration_type: &str) -> Option<&'static str> {
    Some(match decoration_type {
        "minecraft:mansion" => "minecraft:woodland_explorer_map",
        "minecraft:monument" => "minecraft:ocean_explorer_map",
        "minecraft:trial_chambers" => "minecraft:trial_explorer_map",
        "minecraft:jungle_temple" => "minecraft:jungle_explorer_map",
        "minecraft:swamp_hut" => "minecraft:swamp_explorer_map",
        "minecraft:village_desert" => "minecraft:desert_village_map",
        "minecraft:village_plains" => "minecraft:plains_village_map",
        "minecraft:village_savanna" => "minecraft:savanna_village_map",
        "minecraft:village_snowy" => "minecraft:snowy_village_map",
        "minecraft:village_taiga" => "minecraft:taiga_village_map",
        "minecraft:red_x" => "minecraft:buried_treasure_map",
        _ => return None,
    })
}

fn decoration_type_to_legacy_name_key(decoration_type: &str) -> Option<&'static str> {
    Some(match decoration_type {
        "minecraft:mansion" => "filled_map.mansion",
        "minecraft:monument" => "filled_map.monument",
        "minecraft:trial_chambers" => "filled_map.trial_chambers",
        "minecraft:jungle_temple" => "filled_map.explorer_jungle",
        "minecraft:swamp_hut" => "filled_map.explorer_swamp",
        "minecraft:village_desert" => "filled_map.village_desert",
        "minecraft:village_plains" => "filled_map.village_plains",
        "minecraft:village_savanna" => "filled_map.village_savanna",
        "minecraft:village_snowy" => "filled_map.village_snowy",
        "minecraft:village_taiga" => "filled_map.village_taiga",
        "minecraft:red_x" => "filled_map.buried_treasure",
        _ => return None,
    })
}

pub struct ExplorerMapItemFix {
    base: DataFixBase,
}

impl ExplorerMapItemFix {
    pub fn new(output_schema: &Arc<Schema>) -> ExplorerMapItemFix {
        ExplorerMapItemFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn remove_legacy_item_name(components: Dynamic, legacy_name_key: Option<&str>) -> Dynamic {
    let holds_legacy_name = components
        .get(ITEM_NAME_KEY)
        .result()
        .and_then(|name| extra_data_fix_utils::get_plain_translation_key(&name))
        .is_some_and(|name_key| Some(name_key.as_str()) == legacy_name_key);
    if holds_legacy_name { components.remove(ITEM_NAME_KEY) } else { components }
}

impl DataFix for ExplorerMapItemFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let components_finder = find_field(&item_stack_type, "components");
        self.fix_type_everywhere_typed("ExplorerMapItemFix", item_stack_type, move |item_stack| {
            let id = item_stack.get_optional(&id_finder);
            if id.as_ref().and_then(Value::as_str) != Some("minecraft:filled_map") {
                return item_stack;
            }
            let Some(components) = item_stack.get_optional_typed(&components_finder) else { return item_stack };
            let components_data = match components.get_or_create_typed(&remainder_finder()).value {
                Value::Dynamic(dynamic) => dynamic,
                _ => Dynamic::empty(),
            };
            let Some(map_decorations) = components_data.get("minecraft:map_decorations").result() else { return item_stack };
            let Some(explorer_decoration) = map_decorations.get(DECORATION_KEY).result() else { return item_stack };
            let decoration_type = ensure_namespaced(&explorer_decoration.get("type").as_string_or(""));
            let Some(new_item_id) = decoration_type_to_item_id(&decoration_type) else { return item_stack };
            let legacy_name_key = decoration_type_to_legacy_name_key(&decoration_type);
            let ty = item_stack.ty.clone();
            crate::util::write_and_read_typed_or_throw(&item_stack, &ty, |data| {
                let id = data.create_string(new_item_id);
                data.set("id", id).update("components", |component_data| remove_legacy_item_name(component_data, legacy_name_key))
            })
        })
    }
}
