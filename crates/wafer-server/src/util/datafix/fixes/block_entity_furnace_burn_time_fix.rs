use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityFurnaceBurnTimeFix;

impl BlockEntityFurnaceBurnTimeFix {
    pub fn new(output_schema: &Arc<Schema>, entity_type: &str) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, &format!("BlockEntityFurnaceBurnTimeFix{entity_type}"), BLOCK_ENTITY, entity_type, fix)
    }
}

fn fix_burn_time(data: Dynamic) -> Dynamic {
    let data = data.rename_field("CookTime", "cooking_time_spent");
    let data = data.rename_field("CookTimeTotal", "cooking_total_time");
    let data = data.rename_field("BurnTime", "lit_time_remaining");
    let lit_time_remaining = data.get("lit_time_remaining").result();
    data.set_field_if_present("lit_total_time", lit_time_remaining)
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_burn_time)
}
