use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, find_field};
use crate::datafixers::types::Type;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;
use crate::util::java_collections::JavaHashSet;

const CONVERTED_ADDITIONAL_TOOLTIP_TYPES: [&str; 17] = [
    "minecraft:banner_patterns",
    "minecraft:bees",
    "minecraft:block_entity_data",
    "minecraft:block_state",
    "minecraft:bundle_contents",
    "minecraft:charged_projectiles",
    "minecraft:container",
    "minecraft:container_loot",
    "minecraft:firework_explosion",
    "minecraft:fireworks",
    "minecraft:instrument",
    "minecraft:map_id",
    "minecraft:painting/variant",
    "minecraft:pot_decorations",
    "minecraft:potion_contents",
    "minecraft:tropical_fish/pattern",
    "minecraft:written_book_content",
];

pub struct TooltipDisplayComponentFix {
    base: DataFixBase,
}

impl TooltipDisplayComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> TooltipDisplayComponentFix {
        TooltipDisplayComponentFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for TooltipDisplayComponentFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let components_type = self.get_input_schema().get_type(DATA_COMPONENTS);
        let new_components_type = self.get_output_schema().get_type(DATA_COMPONENTS);
        let can_place_on_finder = find_field(&components_type, "minecraft:can_place_on");
        let can_break_finder = find_field(&components_type, "minecraft:can_break");
        let new_can_place_on_type = new_components_type.find_field_type("minecraft:can_place_on").expect("a can_place_on field");
        let new_can_break_type = new_components_type.find_field_type("minecraft:can_break").expect("a can_break field");
        self.fix_type_everywhere_typed_to("TooltipDisplayComponentFix", components_type, new_components_type, move |typed| {
            fix(typed, &can_place_on_finder, &can_break_finder, &new_can_place_on_type, &new_can_break_type)
        })
    }
}

fn fix(typed: Typed, can_place_on_finder: &OpticFinder, can_break_finder: &OpticFinder, new_can_place_on_type: &Arc<Type>, new_can_break_type: &Arc<Type>) -> Typed {
    let mut hidden_tooltips = JavaHashSet::default();
    let typed = fix_adventure_mode_predicate(typed, can_place_on_finder, new_can_place_on_type, "minecraft:can_place_on", &mut hidden_tooltips);
    let typed = fix_adventure_mode_predicate(typed, can_break_finder, new_can_break_type, "minecraft:can_break", &mut hidden_tooltips);
    typed.update_remainder(|remainder| {
        let mut remainder = fix_simple_component(remainder, "minecraft:trim", &mut hidden_tooltips);
        remainder = fix_simple_component(remainder, "minecraft:unbreakable", &mut hidden_tooltips);
        remainder = fix_component_and_unwrap(remainder, "minecraft:dyed_color", "rgb", &mut hidden_tooltips);
        remainder = fix_component_and_unwrap(remainder, "minecraft:attribute_modifiers", "modifiers", &mut hidden_tooltips);
        remainder = fix_component_and_unwrap(remainder, "minecraft:enchantments", "levels", &mut hidden_tooltips);
        remainder = fix_component_and_unwrap(remainder, "minecraft:stored_enchantments", "levels", &mut hidden_tooltips);
        remainder = fix_component_and_unwrap(remainder, "minecraft:jukebox_playable", "song", &mut hidden_tooltips);
        let hide_tooltip = remainder.get("minecraft:hide_tooltip").result().is_some();
        remainder = remainder.remove("minecraft:hide_tooltip");
        let hide_additional_tooltip = remainder.get("minecraft:hide_additional_tooltip").result().is_some();
        remainder = remainder.remove("minecraft:hide_additional_tooltip");
        if hide_additional_tooltip {
            for component_id in CONVERTED_ADDITIONAL_TOOLTIP_TYPES {
                if remainder.get(component_id).result().is_some() {
                    hidden_tooltips.insert(component_id.to_string());
                }
            }
        }
        if hidden_tooltips.is_empty() && !hide_tooltip {
            remainder
        } else {
            let tooltip_display = remainder.create_map([
                (remainder.create_string("hide_tooltip"), remainder.create_boolean(hide_tooltip)),
                (remainder.create_string("hidden_components"), remainder.create_list(hidden_tooltips.iter().map(|component_id| remainder.create_string(component_id)))),
            ]);
            remainder.set("minecraft:tooltip_display", tooltip_display)
        }
    })
}

fn fix_simple_component(remainder: Dynamic, component_id: &str, hidden_tooltips: &mut JavaHashSet) -> Dynamic {
    fix_remainder_component(remainder, component_id, hidden_tooltips, |component| component)
}

fn fix_component_and_unwrap(remainder: Dynamic, component_id: &str, field_name: &str, hidden_tooltips: &mut JavaHashSet) -> Dynamic {
    fix_remainder_component(remainder, component_id, hidden_tooltips, |component| component.get(field_name).result().unwrap_or(component))
}

fn fix_remainder_component(remainder: Dynamic, component_id: &str, hidden_tooltips: &mut JavaHashSet, fixer: impl FnOnce(Dynamic) -> Dynamic) -> Dynamic {
    remainder.update(component_id, |component| {
        let show_in_tooltip = component.get("show_in_tooltip").as_boolean_or(true);
        if !show_in_tooltip {
            hidden_tooltips.insert(component_id.to_string());
        }
        fixer(component.remove("show_in_tooltip"))
    })
}

fn fix_adventure_mode_predicate(typed_components: Typed, component_finder: &OpticFinder, new_type: &Arc<Type>, component_id: &str, hidden_tooltips: &mut JavaHashSet) -> Typed {
    typed_components.update_typed(component_finder, Some(new_type), |typed_component| {
        crate::util::write_and_read_typed_or_throw(&typed_component, new_type, |component| {
            let predicates = component.get("predicates");
            let Some(predicates) = predicates.result() else { return component };
            let show_in_tooltip = component.get("show_in_tooltip").as_boolean_or(true);
            if !show_in_tooltip {
                hidden_tooltips.insert(component_id.to_string());
            }
            predicates
        })
    })
}
