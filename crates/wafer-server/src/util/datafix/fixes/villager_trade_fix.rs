use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::util::datafix::fixes::references::{ITEM_NAME, VILLAGER_TRADE};

pub struct VillagerTradeFix {
    base: DataFixBase,
}

impl VillagerTradeFix {
    pub fn new(output_schema: &Arc<Schema>) -> VillagerTradeFix {
        VillagerTradeFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for VillagerTradeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let recipe_type = self.get_input_schema().get_type(VILLAGER_TRADE);
        let buy_finder = find_field(&recipe_type, "buy");
        let buy_b_finder = find_field(&recipe_type, "buyB");
        let sell_finder = find_field(&recipe_type, "sell");
        let id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
        self.fix_type_everywhere_typed("Villager trade fix", recipe_type, move |recipe| {
            recipe
                .update_typed(&buy_finder, None, |item_stack| update_item_stack(&id_finder, item_stack))
                .update_typed(&buy_b_finder, None, |item_stack| update_item_stack(&id_finder, item_stack))
                .update_typed(&sell_finder, None, |item_stack| update_item_stack(&id_finder, item_stack))
        })
    }
}

fn update_item_stack(id_finder: &OpticFinder, item_stack: Typed) -> Typed {
    item_stack.update(id_finder, |pair| pair.map_string(|name| if name == "minecraft:carved_pumpkin" { "minecraft:pumpkin".to_string() } else { name.to_string() }))
}
