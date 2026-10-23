use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct EntityAttributeBaseValueFix;

impl EntityAttributeBaseValueFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        name: &str,
        entity_name: &str,
        attribute_id: &'static str,
        value_fixer: impl Fn(f64) -> f64 + Send + Sync + 'static,
    ) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, name, ENTITY, entity_name, move |entity: Typed| entity.update_remainder(|tag| fix_value(attribute_id, &value_fixer, tag)))
    }
}

fn fix_value(attribute_id: &str, value_fixer: &impl Fn(f64) -> f64, tag: Dynamic) -> Dynamic {
    tag.clone().update("attributes", |attributes| {
        tag.create_list(attributes.as_stream().into_iter().map(|attribute| {
            let id = ensure_namespaced(&attribute.get("id").as_string_or(""));
            if id != attribute_id {
                return attribute;
            }
            let base = attribute.get("base").as_double(0.0);
            let fixed_base = attribute.create_double(value_fixer(base));
            attribute.set("base", fixed_base)
        }))
    })
}
