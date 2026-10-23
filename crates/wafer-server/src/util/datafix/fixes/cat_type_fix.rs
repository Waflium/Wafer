use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct CatTypeFix;

impl CatTypeFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "CatTypeFix", ENTITY, "minecraft:cat", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    if input.get("CatType").as_int(0) == 9 {
        let cat_type = input.create_int(10);
        input.set("CatType", cat_type)
    } else {
        input
    }
}
