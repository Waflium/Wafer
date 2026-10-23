use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;

pub struct AddFieldFix {
    base: DataFixBase,
    name: String,
    ty: TypeReference,
    field_name: String,
    path: Arc<Vec<String>>,
    field_generator: Arc<dyn Fn(&Dynamic) -> Dynamic + Send + Sync>,
}

impl AddFieldFix {
    pub fn new(output_schema: &Arc<Schema>, ty: TypeReference, field_name: &str, field_generator: impl Fn(&Dynamic) -> Dynamic + Send + Sync + 'static, path: &[&str]) -> AddFieldFix {
        AddFieldFix {
            base: DataFixBase::new(output_schema, false),
            name: format!("Adding field `{field_name}` to type `{}`", ty.type_name().to_lowercase()),
            ty,
            field_name: field_name.to_string(),
            path: Arc::new(path.iter().map(|segment| segment.to_string()).collect()),
            field_generator: Arc::new(field_generator),
        }
    }
}

impl DataFix for AddFieldFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let (field_name, path, field_generator) = (self.field_name.clone(), self.path.clone(), self.field_generator.clone());
        self.fix_type_everywhere_typed_to(&self.name, self.get_input_schema().get_type(self.ty), self.get_output_schema().get_type(self.ty), move |input| {
            input.update_remainder(|dynamic| add_field(&field_name, &path, &*field_generator, dynamic, 0))
        })
    }
}

fn add_field(field_name: &str, path: &[String], field_generator: &dyn Fn(&Dynamic) -> Dynamic, dynamic: Dynamic, path_index: usize) -> Dynamic {
    if path_index >= path.len() {
        let field = field_generator(&dynamic);
        return dynamic.set(field_name, field);
    }
    match dynamic.get(&path[path_index]).result() {
        Some(field) => add_field(field_name, path, field_generator, field, path_index + 1),
        None => dynamic,
    }
}
