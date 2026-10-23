use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

const CAN_PICK_UP_LOOT: &str = "CanPickUpLoot";

pub struct VillagerSetCanPickUpLootFix;

impl VillagerSetCanPickUpLootFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, true, "Villager CanPickUpLoot default value", ENTITY, "Villager", |entity| entity.update_remainder(fix_value))
    }
}

fn fix_value(tag: Dynamic) -> Dynamic {
    let value = tag.create_boolean(true);
    tag.set(CAN_PICK_UP_LOOT, value)
}
