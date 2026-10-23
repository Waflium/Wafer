use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::SAVED_DATA_WORLD_BORDER;

pub struct WorldBorderWarningTimeFix {
    base: DataFixBase,
}

impl WorldBorderWarningTimeFix {
    pub fn new(output_schema: &Arc<Schema>) -> WorldBorderWarningTimeFix {
        WorldBorderWarningTimeFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for WorldBorderWarningTimeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.write_fix_and_read(
            "WorldBorderWarningTimeFix",
            self.get_input_schema().get_type(SAVED_DATA_WORLD_BORDER),
            self.get_output_schema().get_type(SAVED_DATA_WORLD_BORDER),
            |input| {
                input.update("data", |tag| {
                    let tag_for_create = tag.clone();
                    tag.update("warning_time", |warning_time| tag_for_create.create_int(warning_time.as_int(15) * 20))
                })
            },
        )
    }
}
