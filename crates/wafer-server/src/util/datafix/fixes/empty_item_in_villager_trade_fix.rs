use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::VILLAGER_TRADE;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct EmptyItemInVillagerTradeFix {
    base: DataFixBase,
}

impl EmptyItemInVillagerTradeFix {
    pub fn new(output_schema: &Arc<Schema>) -> EmptyItemInVillagerTradeFix {
        EmptyItemInVillagerTradeFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for EmptyItemInVillagerTradeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let trade_type = self.get_input_schema().get_type(VILLAGER_TRADE);
        self.write_fix_and_read("EmptyItemInVillagerTradeFix", trade_type.clone(), trade_type, |input| {
            let buy_b = input.get("buyB").or_else_empty_map();
            let id = ensure_namespaced(&buy_b.get("id").as_string_or("minecraft:air"));
            let count = buy_b.get("count").as_int(0);
            if id != "minecraft:air" && count != 0 { input } else { input.remove("buyB") }
        })
    }
}
