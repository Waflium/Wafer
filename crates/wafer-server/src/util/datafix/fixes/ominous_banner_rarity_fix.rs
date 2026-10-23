use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field, type_finder};
use crate::datafixers::types::{const_type, named};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, ITEM_NAME, ITEM_STACK, TEXT_COMPONENT};
use crate::util::datafix::legacy_component_data_fix_utils;
use wafer_protocol::nbt::Nbt;

pub struct OminousBannerRarityFix {
    base: DataFixBase,
}

impl OminousBannerRarityFix {
    pub fn new(output_schema: &Arc<Schema>) -> OminousBannerRarityFix {
        OminousBannerRarityFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for OminousBannerRarityFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let block_entity_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let block_entity_id_finder = type_finder(self.get_input_schema().find_choice_type(BLOCK_ENTITY));
        let item_stack_id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let block_entity_components_field_finder = find_field(&block_entity_type, "components");
        let item_stack_components_field_finder = find_field(&item_stack_type, "components");
        let item_name_finder = find_field(&block_entity_components_field_finder.ty(), "minecraft:item_name");
        let text_component_finder = type_finder(self.get_input_schema().get_type(TEXT_COMPONENT));
        let (item_name_finder_entity, text_component_finder_entity) = (item_name_finder.clone(), text_component_finder.clone());
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed("Ominous Banner block entity common rarity to uncommon rarity fix", block_entity_type, move |input| {
                let block_entity_id = match input.get_optional(&block_entity_id_finder) {
                    Some(Value::Choice(key, _)) => key,
                    _ => panic!("No block entity id in {:?}", input.ty),
                };
                if block_entity_id == "minecraft:banner" {
                    fix(input, &block_entity_components_field_finder, &item_name_finder_entity, &text_component_finder_entity)
                } else {
                    input
                }
            }),
            self.fix_type_everywhere_typed("Ominous Banner item stack common rarity to uncommon rarity fix", item_stack_type, move |input| {
                let item_stack_id = input.get_optional(&item_stack_id_finder).and_then(|id| id.as_str().map(str::to_string)).unwrap_or_default();
                if item_stack_id == "minecraft:white_banner" {
                    fix(input, &item_stack_components_field_finder, &item_name_finder, &text_component_finder)
                } else {
                    input
                }
            }),
        ])
    }
}

fn fix(input: Typed, components_field_finder: &OpticFinder, item_name_finder: &OpticFinder, text_component_finder: &OpticFinder) -> Typed {
    input.update_typed(components_field_finder, None, |components| {
        let is_ominous_banner = components
            .get_optional_typed(item_name_finder)
            .and_then(|item_name| item_name.get_optional(text_component_finder))
            .and_then(|text_component| text_component.as_str().map(str::to_string))
            .and_then(|text_component| legacy_component_data_fix_utils::extract_translation_string(&text_component))
            .is_some_and(|translation| translation == "block.minecraft.ominous_banner");
        if is_ominous_banner {
            components
                .update_typed(item_name_finder, None, |item_name| {
                    let component = legacy_component_data_fix_utils::create_translatable_component_json("block.minecraft.ominous_banner");
                    item_name.set(text_component_finder, Typed::new(text_component_finder.ty(), Value::Prim(Nbt::String(component))))
                })
                .update_remainder(|remainder| {
                    let rarity = remainder.create_string("uncommon");
                    remainder.set("minecraft:rarity", rarity)
                })
        } else {
            components
        }
    })
}
