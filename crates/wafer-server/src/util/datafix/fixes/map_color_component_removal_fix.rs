use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::data_component_remainder_fix::DataComponentRemainderFix;

pub struct MapColorComponentRemovalFix;

impl MapColorComponentRemovalFix {
    pub fn new(output_schema: &Arc<Schema>) -> DataComponentRemainderFix {
        DataComponentRemainderFix::new(output_schema, "MapColorComponentRemovalFix", "minecraft:map_color", |_| None)
    }
}
