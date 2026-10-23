use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::type_finder;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{HOTBAR, ITEM_STACK};

pub struct EmptyItemInHotbarFix {
    base: DataFixBase,
}

impl EmptyItemInHotbarFix {
    pub fn new(output_schema: &Arc<Schema>) -> EmptyItemInHotbarFix {
        EmptyItemInHotbarFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn fix_item_stack(item_stack: Value) -> Value {
    let Value::Pair(id, rest) = item_stack else { return item_stack };
    let Value::Pair(tag, remainder) = *rest else { return Value::Pair(id, rest) };
    let Value::Dynamic(remainder) = *remainder else { return Value::Pair(id, Box::new(Value::Pair(tag, remainder))) };
    let id_string = match &*id {
        Value::Left(id) => id.as_str().map(str::to_string),
        _ => None,
    };
    let is_air = id_string.as_deref().is_none_or(|id| id == "minecraft:air");
    let is_empty = remainder.get("Count").as_int(0) <= 0;
    if !is_air && !is_empty {
        Value::Pair(id, Box::new(Value::Pair(tag, Box::new(Value::Dynamic(remainder)))))
    } else {
        let empty_map = remainder.empty_map();
        Value::Pair(Box::new(Value::Right(Box::new(Value::Unit))), Box::new(Value::Pair(Box::new(Value::Right(Box::new(Value::Unit))), Box::new(Value::Dynamic(empty_map)))))
    }
}

impl DataFix for EmptyItemInHotbarFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let item_stack_f = type_finder(self.get_input_schema().get_type(ITEM_STACK));
        self.fix_type_everywhere_typed("EmptyItemInHotbarFix", self.get_input_schema().get_type(HOTBAR), move |input| input.update(&item_stack_f, fix_item_stack))
    }
}
