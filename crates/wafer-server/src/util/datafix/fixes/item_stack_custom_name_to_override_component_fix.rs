use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::util::datafix::fixes::references::{ITEM_NAME, ITEM_STACK};
use crate::util::datafix::legacy_component_data_fix_utils;

const MAP_NAMES: [&str; 11] = [
    "filled_map.buried_treasure",
    "filled_map.explorer_jungle",
    "filled_map.explorer_swamp",
    "filled_map.mansion",
    "filled_map.monument",
    "filled_map.trial_chambers",
    "filled_map.village_desert",
    "filled_map.village_plains",
    "filled_map.village_savanna",
    "filled_map.village_snowy",
    "filled_map.village_taiga",
];

pub struct ItemStackCustomNameToOverrideComponentFix {
    base: DataFixBase,
}

impl ItemStackCustomNameToOverrideComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> ItemStackCustomNameToOverrideComponentFix {
        ItemStackCustomNameToOverrideComponentFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn fix_map(value: Typed) -> Typed {
    fix_custom_name(value, |translation_key| MAP_NAMES.contains(&translation_key))
}

fn fix_banner(value: Typed) -> Typed {
    fix_custom_name(value, |translation_key| translation_key == "block.minecraft.ominous_banner")
}

fn fix_custom_name(typed: Typed, expected_translation_key: impl Fn(&str) -> bool) -> Typed {
    let ty = typed.ty.clone();
    crate::util::write_and_read_typed_or_throw(&typed, &ty, |value| {
        let has_correct_translation_key = value
            .get("minecraft:custom_name")
            .as_string()
            .into_result()
            .ok()
            .and_then(|custom_name| legacy_component_data_fix_utils::extract_translation_string(&custom_name))
            .filter(|translation_key| expected_translation_key(translation_key))
            .is_some();
        if has_correct_translation_key { value.rename_field("minecraft:custom_name", "minecraft:item_name") } else { value }
    })
}

impl DataFix for ItemStackCustomNameToOverrideComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let components_finder = find_field(&item_stack_type, "components");
        self.fix_type_everywhere_typed("ItemStack custom_name to item_name component fix", item_stack_type, move |input| {
            let maybe_id = input.get_optional(&id_finder).and_then(|id| id.as_str().map(str::to_string));
            match maybe_id.as_deref() {
                Some("minecraft:white_banner") => input.update_typed(&components_finder, None, fix_banner),
                Some("minecraft:filled_map") => input.update_typed(&components_finder, None, fix_map),
                _ => input,
            }
        })
    }
}
