use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::named_choice;
use crate::util::datafix::extra_data_fix_utils;

pub type DynamicFix = Arc<dyn Fn(Dynamic) -> Dynamic + Send + Sync>;

pub struct NamedEntityWriteReadFix {
    base: DataFixBase,
    name: String,
    entity_name: String,
    ty: TypeReference,
    fix: DynamicFix,
}

impl NamedEntityWriteReadFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        changes_type: bool,
        name: &str,
        ty: TypeReference,
        entity_name: &str,
        fix: impl Fn(Dynamic) -> Dynamic + Send + Sync + 'static,
    ) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix { base: DataFixBase::new(output_schema, changes_type), name: name.to_string(), entity_name: entity_name.to_string(), ty, fix: Arc::new(fix) }
    }
}

impl DataFix for NamedEntityWriteReadFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_entity_type = self.get_input_schema().get_type(self.ty);
        let input_entity_choice_type = self.get_input_schema().get_choice_type(self.ty, &self.entity_name);
        let output_entity_type = self.get_output_schema().get_type(self.ty);
        let entity_finder = named_choice(&self.entity_name, input_entity_choice_type);
        let patched_entity_type = extra_data_fix_utils::patch_sub_type(&input_entity_type, &input_entity_type, &output_entity_type);
        let output_type = output_entity_type.clone();
        let fix = self.fix.clone();
        self.fix_type_everywhere_typed_to(&self.name, input_entity_type, output_entity_type, move |typed| {
            if typed.get_optional(&entity_finder).is_none() {
                return extra_data_fix_utils::cast(&output_type, typed);
            }
            let fake_typed = extra_data_fix_utils::cast(&patched_entity_type, typed);
            crate::util::write_and_read_typed_or_throw(&fake_typed, &output_type, |dynamic| fix(dynamic))
        })
    }
}
