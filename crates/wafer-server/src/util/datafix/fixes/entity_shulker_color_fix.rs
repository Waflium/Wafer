use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityShulkerColorFix;

impl EntityShulkerColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "EntityShulkerColorFix", ENTITY, "minecraft:shulker", |entity| entity.update_remainder(fix_tag))
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    if input.get("Color").result().is_none() {
        let color = input.create_byte(10);
        input.set("Color", color)
    } else {
        input
    }
}
