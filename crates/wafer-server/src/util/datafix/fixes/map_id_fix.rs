use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::SAVED_DATA_MAP_INDEX;

pub struct MapIdFix {
    base: DataFixBase,
}

impl MapIdFix {
    pub fn new(schema: &Arc<Schema>) -> MapIdFix {
        MapIdFix { base: DataFixBase::new(schema, false) }
    }
}

impl DataFix for MapIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("Map id fix", self.get_input_schema().get_type(SAVED_DATA_MAP_INDEX), |input| {
            input.update_remainder(|tag| tag.create_map([(tag.create_string("data"), tag.clone())]))
        })
    }
}
