use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityItemFrameDirectionFix;

impl EntityItemFrameDirectionFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "EntityItemFrameDirectionFix", ENTITY, "minecraft:item_frame", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    let facing = input.create_byte(direction_2d_to_3d(input.get("Facing").as_byte(0)));
    input.set("Facing", facing)
}

fn direction_2d_to_3d(dir: i8) -> i8 {
    match dir {
        0 => 3,
        1 => 4,
        3 => 5,
        _ => 2,
    }
}
