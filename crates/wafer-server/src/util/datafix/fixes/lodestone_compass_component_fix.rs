use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::data_component_remainder_fix::DataComponentRemainderFix;

pub struct LodestoneCompassComponentFix;

impl LodestoneCompassComponentFix {
    pub fn new(output_schema: &Arc<Schema>) -> DataComponentRemainderFix {
        DataComponentRemainderFix::renaming(output_schema, "LodestoneCompassComponentFix", "minecraft:lodestone_target", "minecraft:lodestone_tracker", fix_component)
    }
}

fn fix_component(mut input: Dynamic) -> Option<Dynamic> {
    let pos = input.get("pos").result();
    let dimension = input.get("dimension").result();
    input = input.remove("pos").remove("dimension");
    if let (Some(pos), Some(dimension)) = (pos, dimension) {
        let target = input.empty_map().set("pos", pos).set("dimension", dimension);
        input = input.set("target", target);
    }
    Some(input)
}
