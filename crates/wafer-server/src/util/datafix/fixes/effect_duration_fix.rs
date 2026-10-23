use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::util::datafix::fixes::references::{ENTITY, ITEM_NAME, ITEM_STACK, PLAYER};

const POTION_ITEMS: [&str; 4] = ["minecraft:potion", "minecraft:splash_potion", "minecraft:lingering_potion", "minecraft:tipped_arrow"];

pub struct EffectDurationFix {
    base: DataFixBase,
}

impl EffectDurationFix {
    pub fn new(output_schema: &Arc<Schema>) -> EffectDurationFix {
        EffectDurationFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for EffectDurationFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema();
        let item_stack_type = input_schema.get_type(ITEM_STACK);
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        let tag_finder = find_field(&item_stack_type, "tag");
        TypeRewriteRule::seq(vec![
            self.fix_type_everywhere_typed("EffectDurationEntity", input_schema.get_type(ENTITY), |input| input.update_remainder(update_entity)),
            self.fix_type_everywhere_typed("EffectDurationPlayer", input_schema.get_type(PLAYER), |input| input.update_remainder(update_entity)),
            self.fix_type_everywhere_typed("EffectDurationItem", item_stack_type, move |input| {
                if input.get_optional(&id_finder).is_some_and(|id| id.as_str().is_some_and(|id| POTION_ITEMS.contains(&id))) {
                    if let Some(tag) = input.get_optional_typed(&tag_finder) {
                        let tag_rest = tag.get_remainder();
                        let new_tag = tag.set_remainder(tag_rest.update("CustomPotionEffects", fix));
                        return input.set(&tag_finder, new_tag);
                    }
                }
                input
            }),
        ])
    }
}

fn fix_effect(effect: Dynamic) -> Dynamic {
    let duration = effect.get("Duration").as_int(-1);
    effect.update("FactorCalculationData", |factor_data| {
        let timestamp = factor_data.get("effect_changed_timestamp").as_int(-1);
        let factor_data = factor_data.remove("effect_changed_timestamp");
        let ticks_active = timestamp.wrapping_sub(duration);
        let ticks_active = factor_data.create_int(ticks_active);
        factor_data.set("ticks_active", ticks_active)
    })
}

fn fix(input: Dynamic) -> Dynamic {
    let effects = input.as_stream().into_iter().map(fix_effect);
    input.create_list(effects)
}

fn update_entity(mut data: Dynamic) -> Dynamic {
    data = data.update("Effects", fix);
    data = data.update("ActiveEffects", fix);
    data.update("CustomPotionEffects", fix)
}
