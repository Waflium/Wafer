use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, named_choice};

pub type EntityFix = Arc<dyn Fn(Typed) -> Typed + Send + Sync>;

pub struct NamedEntityFix {
    base: DataFixBase,
    name: String,
    pub entity_name: String,
    pub ty: TypeReference,
    fix: EntityFix,
}

impl NamedEntityFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        changes_type: bool,
        name: &str,
        ty: TypeReference,
        entity_name: &str,
        fix: impl Fn(Typed) -> Typed + Send + Sync + 'static,
    ) -> NamedEntityFix {
        NamedEntityFix { base: DataFixBase::new(output_schema, changes_type), name: name.to_string(), entity_name: entity_name.to_string(), ty, fix: Arc::new(fix) }
    }
}

impl DataFix for NamedEntityFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let entity_finder = named_choice(&self.entity_name, self.get_input_schema().get_choice_type(self.ty, &self.entity_name));
        let output_choice_type = self.get_output_schema().get_choice_type(self.ty, &self.entity_name);
        let fix = self.fix.clone();
        self.fix_type_everywhere_typed_to(&self.name, self.get_input_schema().get_type(self.ty), self.get_output_schema().get_type(self.ty), move |input| {
            input.update_typed(&entity_finder, Some(&output_choice_type), |entity| fix(entity))
        })
    }
}
