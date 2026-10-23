use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::legacy_abstract_block_property_fix::LegacyAbstractBlockPropertyFix;

pub struct LegacyBlockPropertyRenameAndFix;

impl LegacyBlockPropertyRenameAndFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        name: &str,
        block_id: &'static str,
        old_property_name: &'static str,
        new_property_name: &'static str,
        value_fixer: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> LegacyAbstractBlockPropertyFix {
        LegacyAbstractBlockPropertyFix::new(
            output_schema,
            name,
            move |id| id == block_id,
            move |_, properties| properties.rename_and_fix_field(old_property_name, new_property_name, |dynamic| dynamic.create_string(value_fixer(&dynamic.as_string_or("")))),
        )
    }
}
