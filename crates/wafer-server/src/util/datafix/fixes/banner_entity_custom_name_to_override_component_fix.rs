use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, find_field, type_finder};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, TEXT_COMPONENT};
use crate::util::datafix::legacy_component_data_fix_utils;

pub struct BannerEntityCustomNameToOverrideComponentFix {
    base: DataFixBase,
}

impl BannerEntityCustomNameToOverrideComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> BannerEntityCustomNameToOverrideComponentFix {
        BannerEntityCustomNameToOverrideComponentFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn fix(input: Typed, text_component_finder: &OpticFinder, custom_name_finder: &OpticFinder) -> Typed {
    let custom_name = input
        .get_optional_typed(custom_name_finder)
        .and_then(|name| name.get_optional(text_component_finder))
        .and_then(|text_component| text_component.as_str().map(str::to_string));
    let is_ominous_banner = custom_name
        .as_deref()
        .and_then(legacy_component_data_fix_utils::extract_translation_string)
        .is_some_and(|translation_key| translation_key == "block.minecraft.ominous_banner");
    if !is_ominous_banner {
        return input;
    }
    let ty = input.ty.clone();
    crate::util::write_and_read_typed_or_throw(&input, &ty, |dynamic: Dynamic| {
        let components = dynamic.create_map([
            (dynamic.create_string("minecraft:item_name"), dynamic.create_string(custom_name.clone().expect("a custom name"))),
            (dynamic.create_string("minecraft:hide_additional_tooltip"), dynamic.empty_map()),
        ]);
        dynamic.set("components", components).remove("CustomName")
    })
}

impl DataFix for BannerEntityCustomNameToOverrideComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let block_entity_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let block_entity_id_finder = type_finder(self.get_input_schema().find_choice_type(BLOCK_ENTITY));
        let custom_name_finder = find_field(&block_entity_type, "CustomName");
        let text_component_finder = type_finder(self.get_input_schema().get_type(TEXT_COMPONENT));
        self.fix_type_everywhere_typed("Banner entity custom_name to item_name component fix", block_entity_type, move |input| {
            let block_entity_id = match input.get_typed(&block_entity_id_finder).value {
                Value::Choice(id, _) => id,
                other => panic!("Not a tagged choice: {other:?}"),
            };
            if block_entity_id == "minecraft:banner" { fix(input, &text_component_finder, &custom_name_finder) } else { input }
        })
    }
}
