use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::remainder_finder;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;

pub struct SimpleEntityRenameFix;

impl SimpleEntityRenameFix {
    pub fn new(name: &str, output_schema: &Arc<Schema>, changes_type: bool, get_new_name_and_tag: impl Fn(&str, Dynamic) -> (String, Dynamic) + Send + Sync + 'static) -> EntityRenameFix {
        EntityRenameFix::new(name, output_schema, changes_type, move |name, entity| {
            let tag = match entity.get_or_create_typed(&remainder_finder()).value {
                Value::Dynamic(tag) => tag,
                _ => Dynamic::empty(),
            };
            let (new_name, new_tag) = get_new_name_and_tag(name, tag);
            (new_name, entity.set_remainder(new_tag))
        })
    }
}
