use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;

pub struct WorldSpawnDataFix {
    base: DataFixBase,
}

impl WorldSpawnDataFix {
    pub fn new(output_schema: &Arc<Schema>) -> WorldSpawnDataFix {
        WorldSpawnDataFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for WorldSpawnDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("WorldSpawnDataFix", self.get_input_schema().get_type(LEVEL), |input| {
            input.update_remainder(|tag| {
                let spawn_x = tag.get("SpawnX").as_int(0);
                let spawn_y = tag.get("SpawnY").as_int(0);
                let spawn_z = tag.get("SpawnZ").as_int(0);
                let angle = tag.get("SpawnAngle").as_float(0.0);
                let spawn_data = tag
                    .empty_map()
                    .set("dimension", tag.create_string("minecraft:overworld"))
                    .set("pos", tag.create_int_list([spawn_x, spawn_y, spawn_z]))
                    .set("yaw", tag.create_float(angle))
                    .set("pitch", tag.create_float(0.0));
                let tag = tag.remove("SpawnX").remove("SpawnY").remove("SpawnZ").remove("SpawnAngle");
                tag.set("spawn", spawn_data)
            })
        })
    }
}
