use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::data_component_remainder_fix::DataComponentRemainderFix;

pub struct FireResistantToDamageResistantComponentFix;

impl FireResistantToDamageResistantComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> DataComponentRemainderFix {
        DataComponentRemainderFix::renaming(output_schema, "FireResistantToDamageResistantComponentFix", "minecraft:fire_resistant", "minecraft:damage_resistant", |input| {
            Some(input.empty_map().set("types", input.create_string("#minecraft:is_fire")))
        })
    }
}
