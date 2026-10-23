use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntitySalmonSizeFix;

impl EntitySalmonSizeFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "EntitySalmonSizeFix", ENTITY, "minecraft:salmon", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(|tag| {
        let ty = tag.get("type").as_string_or("medium");
        if ty == "large" {
            return tag;
        }
        let medium = tag.create_string("medium");
        tag.set("type", medium)
    })
}
