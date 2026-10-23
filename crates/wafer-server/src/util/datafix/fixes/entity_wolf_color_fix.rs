use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityWolfColorFix;

impl EntityWolfColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "EntityWolfColorFix", ENTITY, "minecraft:wolf", |entity| entity.update_remainder(fix_tag))
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    input.update("CollarColor", |color| color.create_byte((15 - color.as_int(0)) as i8))
}
