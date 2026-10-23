use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, named_choice, remainder_finder};
use crate::datafixers::types::{Type, and, compound_list, const_type, field, optional, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, RECIPE};

pub struct FurnaceRecipeFix {
    base: DataFixBase,
}

impl FurnaceRecipeFix {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> FurnaceRecipeFix {
        FurnaceRecipeFix { base: DataFixBase::new(schema, changes_type) }
    }
}

impl DataFix for FurnaceRecipeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let recipe_type = self.get_output_schema().get_type_raw(RECIPE);
        let replaced_type = and(optional(field("RecipesUsed", and(compound_list(recipe_type.clone(), const_type(ConstType::Int)), remainder_type()))), remainder_type());
        let old_furnace_finder = named_choice("minecraft:furnace", self.get_input_schema().get_choice_type(BLOCK_ENTITY, "minecraft:furnace"));
        let old_blast_furnace_finder = named_choice("minecraft:blast_furnace", self.get_input_schema().get_choice_type(BLOCK_ENTITY, "minecraft:blast_furnace"));
        let old_smoker_finder = named_choice("minecraft:smoker", self.get_input_schema().get_choice_type(BLOCK_ENTITY, "minecraft:smoker"));
        let new_furnace_type = self.get_output_schema().get_choice_type(BLOCK_ENTITY, "minecraft:furnace");
        let new_blast_furnace_finder = self.get_output_schema().get_choice_type(BLOCK_ENTITY, "minecraft:blast_furnace");
        let new_smoker_finder = self.get_output_schema().get_choice_type(BLOCK_ENTITY, "minecraft:smoker");
        let old_entity_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let new_entity_type = self.get_output_schema().get_type(BLOCK_ENTITY);
        self.fix_type_everywhere_typed_to("FurnaceRecipesFix", old_entity_type, new_entity_type, move |input| {
            input
                .update_typed(&old_furnace_finder, Some(&new_furnace_type), |furnace| update_furnace_contents(&recipe_type, &replaced_type, furnace))
                .update_typed(&old_blast_furnace_finder, Some(&new_blast_furnace_finder), |blast_furnace| update_furnace_contents(&recipe_type, &replaced_type, blast_furnace))
                .update_typed(&old_smoker_finder, Some(&new_smoker_finder), |smoker| update_furnace_contents(&recipe_type, &replaced_type, smoker))
        })
    }
}

fn update_furnace_contents(recipe_type: &Arc<Type>, replaced_type: &Arc<Type>, input: Typed) -> Typed {
    let mut tag = match input.get_or_create_typed(&remainder_finder()).value {
        Value::Dynamic(dynamic) => dynamic,
        _ => crate::datafixers::dynamic::Dynamic::empty(),
    };
    let recipes_used_size = tag.get("RecipesUsedSize").as_int(0);
    tag = tag.remove("RecipesUsedSize");
    let mut results: Vec<(Value, Value)> = Vec::new();
    for i in 0..recipes_used_size {
        let location_key = format!("RecipeLocation{i}");
        let amount_key = format!("RecipeAmount{i}");
        let maybe_location = tag.get(&location_key).result();
        let amount = tag.get(&amount_key).as_int(0);
        if amount > 0
            && let Some(location) = maybe_location
            && let Ok((recipe, _)) = Typed::read_typed(recipe_type, location).into_result()
        {
            results.push((recipe.value, Value::Prim(wafer_protocol::nbt::Nbt::Int(amount))));
        }
        tag = tag.remove(&location_key).remove(&amount_key);
    }
    let empty_map = tag.empty_map();
    let mut value = Some(Value::Pair(Box::new(Value::Left(Box::new(Value::Pair(Box::new(Value::CompoundList(results)), Box::new(Value::Dynamic(empty_map)))))), Box::new(Value::Dynamic(tag))));
    input.update_to(&remainder_finder(), replaced_type, |_| value.take().expect("a single remainder"))
}
