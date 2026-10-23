use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OBJECTIVE;

pub struct ObjectiveRenderTypeFix {
    base: DataFixBase,
}

impl ObjectiveRenderTypeFix {
    pub fn new(output_schema: &Arc<Schema>) -> ObjectiveRenderTypeFix {
        ObjectiveRenderTypeFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn get_render_type(criteria_name: &str) -> &'static str {
    if criteria_name == "health" { "hearts" } else { "integer" }
}

impl DataFix for ObjectiveRenderTypeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let objective_type = self.get_input_schema().get_type(OBJECTIVE);
        self.fix_type_everywhere_typed("ObjectiveRenderTypeFix", objective_type, |typed| {
            typed.update_remainder(|tag| {
                if tag.get("RenderType").as_string().into_result().is_err() {
                    let criteria_name = tag.get("CriteriaName").as_string_or("");
                    let default_render_type = tag.create_string(get_render_type(&criteria_name));
                    tag.set("RenderType", default_render_type)
                } else {
                    tag
                }
            })
        })
    }
}
