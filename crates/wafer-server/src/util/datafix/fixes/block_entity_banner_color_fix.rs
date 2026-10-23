use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityBannerColorFix;

impl BlockEntityBannerColorFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "BlockEntityBannerColorFix", BLOCK_ENTITY, "minecraft:banner", |entity| entity.update_remainder(fix_tag))
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    let input = input.update("Base", |base| base.create_int(15 - base.as_int(0)));
    input.update("Patterns", |list| match list.as_stream_opt().into_result() {
        Ok(stream) => {
            let patterns: Vec<Dynamic> = stream.into_iter().map(|pattern| pattern.update("Color", |color| color.create_int(15 - color.as_int(0)))).collect();
            list.create_list(patterns)
        }
        Err(_) => list,
    })
}
