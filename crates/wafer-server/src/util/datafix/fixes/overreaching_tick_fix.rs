use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

pub struct OverreachingTickFix {
    base: DataFixBase,
}

impl OverreachingTickFix {
    pub fn new(output_schema: &Arc<Schema>) -> OverreachingTickFix {
        OverreachingTickFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for OverreachingTickFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let block_ticks_finder = find_field(&chunk_type, "block_ticks");
        self.fix_type_everywhere_typed("Handle ticks saved in the wrong chunk", chunk_type, move |chunk| {
            let block_ticks = chunk.get_optional_typed(&block_ticks_finder).and_then(|block_ticks| block_ticks.write().into_result().ok());
            chunk.update_remainder(|remainder| {
                let chunk_x = remainder.get("xPos").as_int(0);
                let chunk_z = remainder.get("zPos").as_int(0);
                let fluid_ticks = remainder.get("fluid_ticks").result();
                let remainder = extract_overreaching_ticks(remainder, chunk_x, chunk_z, block_ticks.as_ref(), "neighbor_block_ticks");
                extract_overreaching_ticks(remainder, chunk_x, chunk_z, fluid_ticks.as_ref(), "neighbor_fluid_ticks")
            })
        })
    }
}

fn extract_overreaching_ticks(remainder: Dynamic, chunk_x: i32, chunk_z: i32, ticks: Option<&Dynamic>, name_in_upgrade_data: &str) -> Dynamic {
    let Some(ticks) = ticks else { return remainder };
    let overreaching_ticks: Vec<Dynamic> = ticks
        .as_stream()
        .into_iter()
        .filter(|tick| {
            let x = tick.get("x").as_int(0);
            let z = tick.get("z").as_int(0);
            let dist_x = chunk_x.wrapping_sub(x >> 4).wrapping_abs();
            let dist_z = chunk_z.wrapping_sub(z >> 4).wrapping_abs();
            (dist_x != 0 || dist_z != 0) && dist_x <= 1 && dist_z <= 1
        })
        .collect();
    if overreaching_ticks.is_empty() {
        return remainder;
    }
    let neighbor_ticks = remainder.create_list(overreaching_ticks);
    let upgrade_data = remainder.get("UpgradeData").or_else_empty_map().set(name_in_upgrade_data, neighbor_ticks);
    remainder.set("UpgradeData", upgrade_data)
}
