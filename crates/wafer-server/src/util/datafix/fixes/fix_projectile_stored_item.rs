use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, named_choice};
use crate::datafixers::types::Type;
use crate::util::datafix::extra_data_fix_utils::{self, TypedFilter};
use crate::util::datafix::fixes::references::ENTITY;

const EMPTY_POTION: &str = "minecraft:empty";

type SubFixer = fn(Typed, &Arc<Type>) -> Typed;

pub struct FixProjectileStoredItem {
    base: DataFixBase,
}

impl FixProjectileStoredItem {
    pub fn new(output_schema: &Arc<Schema>) -> FixProjectileStoredItem {
        FixProjectileStoredItem { base: DataFixBase::new(output_schema, true) }
    }

    fn fix_choice(&self, entity_name: &str, fixer: SubFixer) -> TypedFilter {
        let input_entity_choice_type = self.get_input_schema().get_choice_type(ENTITY, entity_name);
        let output_entity_choice_type = self.get_output_schema().get_choice_type(ENTITY, entity_name);
        let entity_f = named_choice(entity_name, input_entity_choice_type);
        Box::new(move |input| input.update_typed(&entity_f, Some(&output_entity_choice_type), |typed| fixer(typed, &output_entity_choice_type)))
    }
}

fn fix_arrow(typed: Typed, output_type: &Arc<Type>) -> Typed {
    crate::util::write_and_read_typed_or_throw(&typed, output_type, |input| {
        let item_stack = create_item_stack(&input, get_arrow_type(&input));
        input.set("item", item_stack)
    })
}

fn get_arrow_type(input: &Dynamic) -> &'static str {
    if input.get("Potion").as_string_or(EMPTY_POTION) == EMPTY_POTION { "minecraft:arrow" } else { "minecraft:tipped_arrow" }
}

fn fix_spectral_arrow(typed: Typed, output_type: &Arc<Type>) -> Typed {
    crate::util::write_and_read_typed_or_throw(&typed, output_type, |input| {
        let item_stack = create_item_stack(&input, "minecraft:spectral_arrow");
        input.set("item", item_stack)
    })
}

fn create_item_stack(input: &Dynamic, item_name: &str) -> Dynamic {
    input.create_map([(input.create_string("id"), input.create_string(item_name)), (input.create_string("Count"), input.create_int(1))])
}

fn cast_unchecked(input: Typed, output_type: &Arc<Type>) -> Typed {
    extra_data_fix_utils::cast(output_type, input)
}

impl DataFix for FixProjectileStoredItem {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_entity_type = self.get_input_schema().get_type(ENTITY);
        let output_entity_type = self.get_output_schema().get_type(ENTITY);
        self.fix_type_everywhere_typed_to(
            "Fix AbstractArrow item type",
            input_entity_type,
            output_entity_type,
            extra_data_fix_utils::chain_all_filters(vec![
                self.fix_choice("minecraft:trident", cast_unchecked),
                self.fix_choice("minecraft:arrow", fix_arrow),
                self.fix_choice("minecraft:spectral_arrow", fix_spectral_arrow),
            ]),
        )
    }
}
