use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct StriderGravityFix;

impl StriderGravityFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "StriderGravityFix", ENTITY, "minecraft:strider", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    if input.get("NoGravity").as_boolean_or(false) {
        let no_gravity = input.create_boolean(false);
        input.set("NoGravity", no_gravity)
    } else {
        input
    }
}
