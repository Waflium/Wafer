use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::attributes_rename_fix::AttributesRenameFix;

pub struct RenameNameplateToNameTagFix;

impl RenameNameplateToNameTagFix {
    pub fn new(output_schema: &Arc<Schema>) -> AttributesRenameFix {
        AttributesRenameFix::new(output_schema, "RenameNameplateToNameTag", rename)
    }
}

fn rename(old_name: &str) -> String {
    if old_name == "minecraft:nameplate_distance" { "minecraft:name_tag_distance".to_string() } else { old_name.to_string() }
}
