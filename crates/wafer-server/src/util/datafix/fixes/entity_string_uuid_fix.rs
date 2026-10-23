use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityStringUuidFix {
    base: DataFixBase,
}

impl EntityStringUuidFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityStringUuidFix {
        EntityStringUuidFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for EntityStringUuidFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("EntityStringUuidFix", self.get_input_schema().get_type(ENTITY), |input| {
            input.update_remainder(|tag| match tag.get("UUID").as_string().into_result() {
                Ok(uuid_string) => {
                    let uuid = crate::commands::arguments::java_uuid_from_string(&uuid_string).unwrap_or_else(|| panic!("Invalid UUID string: {uuid_string}"));
                    let (most, least) = (tag.create_long((uuid >> 64) as i64), tag.create_long(uuid as i64));
                    tag.remove("UUID").set("UUIDMost", most).set("UUIDLeast", least)
                }
                Err(_) => tag,
            })
        })
    }
}
