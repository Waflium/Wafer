use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityFieldsRenameFix;

impl EntityFieldsRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, entity_type: &str, renames: &'static [(&'static str, &'static str)]) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, name, ENTITY, entity_type, move |entity| fix(entity, renames))
    }
}

fn fix_tag(mut data: Dynamic, renames: &[(&str, &str)]) -> Dynamic {
    for (from, to) in renames {
        data = data.rename_field(from, to);
    }
    data
}

fn fix(entity: Typed, renames: &[(&str, &str)]) -> Typed {
    entity.update_remainder(|data| fix_tag(data, renames))
}
