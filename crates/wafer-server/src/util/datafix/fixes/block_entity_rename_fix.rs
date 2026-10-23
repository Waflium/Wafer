use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityRenameFix {
    base: DataFixBase,
    name: String,
    name_change_lookup: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl BlockEntityRenameFix {
    fn new(output_schema: &Arc<Schema>, name: &str, name_change_lookup: impl Fn(&str) -> String + Send + Sync + 'static) -> BlockEntityRenameFix {
        BlockEntityRenameFix { base: DataFixBase::new(output_schema, true), name: name.to_string(), name_change_lookup: Arc::new(name_change_lookup) }
    }

    pub fn create(output_schema: &Arc<Schema>, name: &str, name_change_lookup: impl Fn(&str) -> String + Send + Sync + 'static) -> BlockEntityRenameFix {
        BlockEntityRenameFix::new(output_schema, name, name_change_lookup)
    }
}

impl DataFix for BlockEntityRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().find_choice_type(BLOCK_ENTITY);
        let new_type = self.get_output_schema().find_choice_type(BLOCK_ENTITY);
        let name_change_lookup = self.name_change_lookup.clone();
        self.fix_type_everywhere_to(&self.name, old_type, new_type, move |input| match input {
            Value::Choice(key, value) => Value::Choice(name_change_lookup(&key), value),
            other => other,
        })
    }
}
