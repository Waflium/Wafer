use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;

pub struct LegacyWorldBorderFix {
    base: DataFixBase,
}

impl LegacyWorldBorderFix {
    pub fn new(output_schema: &Arc<Schema>) -> LegacyWorldBorderFix {
        LegacyWorldBorderFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for LegacyWorldBorderFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("LegacyWorldBorderFix", self.get_input_schema().get_type(LEVEL), |input| {
            input.update_remainder(|tag| {
                let world_border = tag
                    .empty_map()
                    .set("center_x", tag.create_double(tag.get("BorderCenterX").as_double(0.0)))
                    .set("center_z", tag.create_double(tag.get("BorderCenterZ").as_double(0.0)))
                    .set("size", tag.create_double(tag.get("BorderSize").as_double(5.999997e7_f32 as f64)))
                    .set("lerp_time", tag.create_long(tag.get("BorderSizeLerpTime").as_long(0)))
                    .set("lerp_target", tag.create_double(tag.get("BorderSizeLerpTarget").as_double(0.0)))
                    .set("safe_zone", tag.create_double(tag.get("BorderSafeZone").as_double(5.0)))
                    .set("damage_per_block", tag.create_double(tag.get("BorderDamagePerBlock").as_double(0.2)))
                    .set("warning_blocks", tag.create_int(tag.get("BorderWarningBlocks").as_int(5)))
                    .set("warning_time", tag.create_int(tag.get("BorderWarningTime").as_int(15)));
                let tag = tag
                    .remove("BorderCenterX")
                    .remove("BorderCenterZ")
                    .remove("BorderSize")
                    .remove("BorderSizeLerpTime")
                    .remove("BorderSizeLerpTarget")
                    .remove("BorderSafeZone")
                    .remove("BorderDamagePerBlock")
                    .remove("BorderWarningBlocks")
                    .remove("BorderWarningTime");
                tag.set("world_border", world_border)
            })
        })
    }
}
