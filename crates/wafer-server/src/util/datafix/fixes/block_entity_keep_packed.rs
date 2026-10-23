use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityKeepPacked;

impl BlockEntityKeepPacked {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(schema, changes_type, "BlockEntityKeepPacked", BLOCK_ENTITY, "DUMMY", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(tag: Dynamic) -> Dynamic {
    let keep_packed = tag.create_boolean(true);
    tag.set("keepPacked", keep_packed)
}
