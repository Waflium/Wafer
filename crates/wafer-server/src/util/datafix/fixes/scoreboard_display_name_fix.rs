use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::util::datafix::fixes::references::TEXT_COMPONENT;
use crate::util::datafix::legacy_component_data_fix_utils;

pub struct ScoreboardDisplayNameFix {
    base: DataFixBase,
    name: String,
    ty: TypeReference,
}

impl ScoreboardDisplayNameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, ty: TypeReference) -> ScoreboardDisplayNameFix {
        ScoreboardDisplayNameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), ty }
    }
}

impl DataFix for ScoreboardDisplayNameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_type = self.get_input_schema().get_type(self.ty);
        let display_name_f = find_field(&input_type, "DisplayName");
        let text_component_f = type_finder(self.get_input_schema().get_type(TEXT_COMPONENT));
        self.fix_type_everywhere_typed(&self.name, input_type, move |team| {
            team.update_typed(&display_name_f, None, |display_name| {
                display_name.update(&text_component_f, |text_component| text_component.map_string(legacy_component_data_fix_utils::create_text_component_json))
            })
        })
    }
}
