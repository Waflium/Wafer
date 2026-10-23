use std::sync::Arc;

use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;

pub struct VariantRenameFix;

impl VariantRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, ty: TypeReference, entity_name: &str, renames: &'static [(&'static str, &'static str)]) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, name, ty, entity_name, move |typed| fix(typed, renames))
    }
}

fn fix(typed: Typed, renames: &[(&str, &str)]) -> Typed {
    typed.update_remainder(|remainder| {
        remainder.update("variant", |variant| match variant.as_string().into_result() {
            Ok(value) => {
                let renamed = renames.iter().find(|(from, _)| *from == value).map_or(value.as_str(), |(_, to)| *to);
                variant.create_string(renamed)
            }
            Err(_) => variant,
        })
    })
}
