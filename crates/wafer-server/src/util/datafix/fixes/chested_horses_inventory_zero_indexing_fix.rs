use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, find_field, named_choice, type_finder};
use crate::datafixers::types::Type;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{ENTITY, ITEM_STACK};

pub struct ChestedHorsesInventoryZeroIndexingFix {
    base: DataFixBase,
}

impl ChestedHorsesInventoryZeroIndexingFix {
    pub fn new(v3807: &Arc<Schema>) -> ChestedHorsesInventoryZeroIndexingFix {
        ChestedHorsesInventoryZeroIndexingFix { base: DataFixBase::new(v3807, false) }
    }

    fn horse_like_inventory_indexing_fixer(&self, item_stack_finder: &OpticFinder, schema: &Arc<Type>, horse_id: &str) -> TypeRewriteRule {
        let choice_type = self.get_input_schema().get_choice_type(ENTITY, horse_id);
        let entity_finder = named_choice(horse_id, choice_type.clone());
        let items_field_finder = find_field(&choice_type, "Items");
        let item_stack_finder = item_stack_finder.clone();
        self.fix_type_everywhere_typed(&format!("Fix non-zero indexing in chest horse type {horse_id}"), schema.clone(), move |input| {
            input.update_typed(&entity_finder, None, |horse_like| {
                horse_like.update_typed(&items_field_finder, None, |items| items.update(&item_stack_finder, fix_named_stack))
            })
        })
    }
}

fn fix_named_stack(named_stack: Value) -> Value {
    let Value::Pair(id, item_stack) = named_stack else { return named_stack };
    let Value::Pair(tag, remainder) = *item_stack else { return Value::Pair(id, item_stack) };
    let Value::Dynamic(remainder) = *remainder else { return Value::Pair(id, Box::new(Value::Pair(tag, remainder))) };
    let remainder = remainder.update("Slot", |slot| slot.create_byte((slot.as_int(2) - 2) as i8));
    Value::Pair(id, Box::new(Value::Pair(tag, Box::new(Value::Dynamic(remainder)))))
}

impl DataFix for ChestedHorsesInventoryZeroIndexingFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_finder = type_finder(self.get_input_schema().get_type(ITEM_STACK));
        let entity_type = self.get_input_schema().get_type(ENTITY);
        TypeRewriteRule::seq(vec![
            self.horse_like_inventory_indexing_fixer(&item_stack_finder, &entity_type, "minecraft:llama"),
            self.horse_like_inventory_indexing_fixer(&item_stack_finder, &entity_type, "minecraft:trader_llama"),
            self.horse_like_inventory_indexing_fixer(&item_stack_finder, &entity_type, "minecraft:mule"),
            self.horse_like_inventory_indexing_fixer(&item_stack_finder, &entity_type, "minecraft:donkey"),
        ])
    }
}
