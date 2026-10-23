use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::data_component_remainder_fix::DataComponentRemainderFix;

pub struct TridentAnimationFix;

impl TridentAnimationFix {
    pub fn new(output_schema: &Arc<Schema>) -> DataComponentRemainderFix {
        DataComponentRemainderFix::new(output_schema, "TridentAnimationFix", "minecraft:consumable", fix_component)
    }
}

fn fix_component(input: Dynamic) -> Option<Dynamic> {
    Some(input.update("animation", |animation| {
        let optional = animation.as_string().into_result().unwrap_or_default();
        if optional == "spear" { animation.create_string("trident") } else { animation }
    }))
}
