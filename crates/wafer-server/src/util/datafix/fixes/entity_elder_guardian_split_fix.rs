use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::simple_entity_rename_fix::SimpleEntityRenameFix;

pub struct EntityElderGuardianSplitFix;

impl EntityElderGuardianSplitFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityRenameFix {
        SimpleEntityRenameFix::new("EntityElderGuardianSplitFix", output_schema, changes_type, |name, tag| {
            let new_name = if name == "Guardian" && tag.get("Elder").as_boolean_or(false) { "ElderGuardian" } else { name };
            (new_name.to_string(), tag)
        })
    }
}
