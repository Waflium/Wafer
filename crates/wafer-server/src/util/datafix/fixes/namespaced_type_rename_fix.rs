use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::{ConstType, TypeReference};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{const_type, equals, named};

pub struct NamespacedTypeRenameFix {
    base: DataFixBase,
    name: String,
    ty: TypeReference,
    renamer: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl NamespacedTypeRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, ty: TypeReference, renamer: impl Fn(&str) -> String + Send + Sync + 'static) -> NamespacedTypeRenameFix {
        NamespacedTypeRenameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), ty, renamer: Arc::new(renamer) }
    }
}

impl DataFix for NamespacedTypeRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let field_type = named(self.ty.type_name(), const_type(ConstType::NamespacedString));
        if !equals(&field_type, &self.get_input_schema().get_type(self.ty), false) {
            panic!("\"{}\" is not what was expected.", self.ty.type_name());
        }
        let renamer = self.renamer.clone();
        self.fix_type_everywhere(&self.name, field_type, move |input| input.map_string(|value| renamer(value)))
    }
}
