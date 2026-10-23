use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::OptionalDynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityRedundantChanceTagsFix {
    base: DataFixBase,
}

impl EntityRedundantChanceTagsFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityRedundantChanceTagsFix {
        EntityRedundantChanceTagsFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for EntityRedundantChanceTagsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("EntityRedundantChanceTagsFix", self.get_input_schema().get_type(ENTITY), |input| {
            input.update_remainder(|mut tag| {
                if is_zero_list(tag.get("HandDropChances"), 2) {
                    tag = tag.remove("HandDropChances");
                }
                if is_zero_list(tag.get("ArmorDropChances"), 4) {
                    tag = tag.remove("ArmorDropChances");
                }
                tag
            })
        })
    }
}

fn is_zero_list(element: OptionalDynamic, size: usize) -> bool {
    let Some(element) = element.result() else { return false };
    let values = element.as_stream_opt();
    if !values.is_success() {
        return false;
    }
    let values = values.value.unwrap_or_default();
    let floats: Vec<Option<f32>> = values.iter().map(|value| value.as_number().into_result().ok().map(|number| number.float_value())).collect();
    if floats.iter().any(Option::is_none) {
        return false;
    }
    floats.len() == size && floats.iter().all(|value| *value == Some(0.0))
}
