use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub struct FoodToConsumableFix {
    base: DataFixBase,
}

impl FoodToConsumableFix {
    pub fn new(output_schema: &Arc<Schema>) -> FoodToConsumableFix {
        FoodToConsumableFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for FoodToConsumableFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.write_fix_and_read("Food to consumable fix", self.get_input_schema().get_type(DATA_COMPONENTS), self.get_output_schema().get_type(DATA_COMPONENTS), fix)
    }
}

fn fix(components: Dynamic) -> Dynamic {
    let Some(food_component) = components.get("minecraft:food").result() else { return components };
    let eat_seconds = food_component.get("eat_seconds").as_float(1.6);
    let effects = food_component.get("effects").as_stream();
    let on_consume_effects: Vec<Dynamic> = effects
        .into_iter()
        .map(|effect| {
            effect
                .empty_map()
                .set("type", effect.create_string("minecraft:apply_effects"))
                .set("effects", effect.create_list(effect.get("effect").result()))
                .set("probability", effect.create_float(effect.get("probability").as_float(1.0)))
        })
        .collect();
    let components = Dynamic::copy_field(&food_component, "using_converts_to", components, "minecraft:use_remainder");
    let components = components.set("minecraft:food", food_component.remove("eat_seconds").remove("effects").remove("using_converts_to"));
    let consumable = components.empty_map().set("consume_seconds", components.create_float(eat_seconds)).set("on_consume_effects", components.create_list(on_consume_effects));
    components.set("minecraft:consumable", consumable)
}
