use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::simplest_entity_rename_fix::SimplestEntityRenameFix;

pub struct EntityTippedArrowFix;

impl EntityTippedArrowFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix::new("EntityTippedArrowFix", output_schema, changes_type, rename)
    }
}

fn rename(name: &str) -> String {
    if name == "TippedArrow" { "Arrow".to_string() } else { name.to_string() }
}
