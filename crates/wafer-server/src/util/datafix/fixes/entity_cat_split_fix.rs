use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::simple_entity_rename_fix::SimpleEntityRenameFix;

pub struct EntityCatSplitFix;

impl EntityCatSplitFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityRenameFix {
        SimpleEntityRenameFix::new("EntityCatSplitFix", output_schema, changes_type, |name, mut tag| {
            if name == "minecraft:ocelot" {
                let ty = tag.get("CatType").as_int(0);
                if ty > 0 && ty < 4 {
                    let cat_type = tag.create_int(ty);
                    tag = tag.set("CatType", cat_type);
                    let owner_uuid = tag.create_string(tag.get("OwnerUUID").as_string_or(""));
                    tag = tag.set("OwnerUUID", owner_uuid);
                    return ("minecraft:cat".to_string(), tag);
                }
            }
            (name.to_string(), tag)
        })
    }
}
