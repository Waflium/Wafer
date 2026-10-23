use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::legacy_abstract_block_property_fix::LegacyAbstractBlockPropertyFix;

const RENAMES: [(&str, &str); 6] = [("down", "down_south"), ("up", "up_north"), ("north", "north_up"), ("south", "south_up"), ("west", "west_up"), ("east", "east_up")];

pub struct LegacyJigsawRotationFix;

impl LegacyJigsawRotationFix {
    pub fn new(output_schema: &Arc<Schema>) -> LegacyAbstractBlockPropertyFix {
        LegacyAbstractBlockPropertyFix::new(output_schema, "jigsaw_rotation_fix", should_fix, |_, properties| fix_properties(properties))
    }
}

fn should_fix(block_id: &str) -> bool {
    block_id == "minecraft:jigsaw"
}

fn fix_properties(properties: Dynamic) -> Dynamic {
    let facing = properties.get("facing").as_string_or("north");
    let orientation = RENAMES.iter().find(|(from, _)| *from == facing).map_or(facing.as_str(), |(_, to)| to);
    let orientation = properties.create_string(orientation);
    properties.remove("facing").set("orientation", orientation)
}
