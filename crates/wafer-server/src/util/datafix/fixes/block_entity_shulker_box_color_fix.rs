use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityShulkerBoxColorFix;

impl BlockEntityShulkerBoxColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "BlockEntityShulkerBoxColorFix", BLOCK_ENTITY, "minecraft:shulker_box", |entity| {
            entity.update_remainder(|tag| tag.remove("Color"))
        })
    }
}
