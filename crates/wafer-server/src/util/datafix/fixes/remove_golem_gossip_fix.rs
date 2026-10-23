use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct RemoveGolemGossipFix;

impl RemoveGolemGossipFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "Remove Golem Gossip Fix", ENTITY, "minecraft:villager", |entity| entity.update_remainder(fix_value))
    }
}

fn fix_value(tag: Dynamic) -> Dynamic {
    tag.update("Gossips", |gossips| gossips.create_list(gossips.as_stream().into_iter().filter(|attribute| attribute.get("Type").as_string_or("") != "golem")))
}
