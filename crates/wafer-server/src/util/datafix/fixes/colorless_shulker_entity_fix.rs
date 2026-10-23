use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct ColorlessShulkerEntityFix;

impl ColorlessShulkerEntityFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "Colorless shulker entity fix", ENTITY, "minecraft:shulker", |entity| {
            entity.update_remainder(|tag| {
                if tag.get("Color").as_int(0) == 10 {
                    let color = tag.create_byte(16);
                    tag.set("Color", color)
                } else {
                    tag
                }
            })
        })
    }
}
