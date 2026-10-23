use std::sync::{Arc, OnceLock};

use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::datafixers::types::Type;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;

pub struct NamedEntityConvertUncheckedFix;

impl NamedEntityConvertUncheckedFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, ty: TypeReference, entity_name: &str) -> NamedEntityFix {
        let schema = output_schema.clone();
        let entity_name_owned = entity_name.to_string();
        let output_type: OnceLock<Arc<Type>> = OnceLock::new();
        NamedEntityFix::new(output_schema, true, name, ty, entity_name, move |entity: Typed| {
            let output_type = output_type.get_or_init(|| schema.get_choice_type(ty, &entity_name_owned));
            extra_data_fix_utils::cast(output_type, entity)
        })
    }
}
