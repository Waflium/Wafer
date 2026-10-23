use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct JigsawPropertiesFix;

impl JigsawPropertiesFix {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(schema, changes_type, "JigsawPropertiesFix", BLOCK_ENTITY, "minecraft:jigsaw", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(tag: Dynamic) -> Dynamic {
    let old_name = tag.get("attachement_type").as_string_or("minecraft:empty");
    let old_pool = tag.get("target_pool").as_string_or("minecraft:empty");
    let name = tag.create_string(old_name.as_str());
    let target = tag.create_string(old_name);
    let pool = tag.create_string(old_pool);
    tag.set("name", name).set("target", target).remove("attachement_type").set("pool", pool).remove("target_pool")
}
