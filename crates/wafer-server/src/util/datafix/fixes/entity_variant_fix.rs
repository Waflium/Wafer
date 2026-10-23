use std::sync::Arc;

use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;

pub struct EntityVariantFix;

impl EntityVariantFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        name: &str,
        ty: TypeReference,
        entity_name: &str,
        field_name: &str,
        id_conversions: impl Fn(i32) -> Option<&'static str> + Send + Sync + 'static,
    ) -> NamedEntityFix {
        let field_name = field_name.to_string();
        NamedEntityFix::new(output_schema, false, name, ty, entity_name, move |typed| fix(typed, &field_name, &id_conversions))
    }
}

fn update_and_rename(input: Dynamic, old_key: &str, new_key: &str, function: impl FnOnce(Dynamic) -> Dynamic) -> Dynamic {
    match input.get(old_key).result() {
        Some(field_value) => input.set(new_key, function(field_value)),
        None => input,
    }
}

fn fix(typed: Typed, field_name: &str, id_conversions: &impl Fn(i32) -> Option<&'static str>) -> Typed {
    typed.update_remainder(|remainder| {
        update_and_rename(remainder, field_name, "variant", |cat_type| match cat_type.as_number().into_result() {
            Ok(number) => {
                let variant = id_conversions(number.int_value()).unwrap_or_else(|| panic!("Cannot invoke \"String.isEmpty()\" because \"data\" is null"));
                cat_type.create_string(variant)
            }
            Err(_) => cat_type,
        })
    })
}
