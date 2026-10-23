use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::simple_entity_rename_fix::SimpleEntityRenameFix;

pub struct EntitySkeletonSplitFix;

impl EntitySkeletonSplitFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityRenameFix {
        SimpleEntityRenameFix::new("EntitySkeletonSplitFix", output_schema, changes_type, |name, tag| {
            let mut name = name;
            if name == "Skeleton" {
                let ty = tag.get("SkeletonType").as_int(0);
                if ty == 1 {
                    name = "WitherSkeleton";
                } else if ty == 2 {
                    name = "Stray";
                }
            }
            (name.to_string(), tag)
        })
    }
}
