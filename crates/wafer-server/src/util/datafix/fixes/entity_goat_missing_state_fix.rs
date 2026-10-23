use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityGoatMissingStateFix;

impl EntityGoatMissingStateFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "EntityGoatMissingStateFix", ENTITY, "minecraft:goat", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(|tag| {
        let has_left_horn = tag.create_boolean(true);
        let has_right_horn = tag.create_boolean(true);
        tag.set("HasLeftHorn", has_left_horn).set("HasRightHorn", has_right_horn)
    })
}
