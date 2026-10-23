use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

const VILLAGER_DATA_FINALIZED: &str = "VillagerDataFinalized";

pub struct VillagerSetVillagerDataFinalized;

impl VillagerSetVillagerDataFinalized {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, true, "Villager VillagerDataFinalized default value", ENTITY, "minecraft:villager", |entity| {
            entity.update_remainder(fix_value)
        })
    }
}

pub fn fix_value(tag: Dynamic) -> Dynamic {
    let finalized = tag.create_boolean(true);
    tag.set(VILLAGER_DATA_FINALIZED, finalized)
}
