use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::BLOCK_STATE;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct LegacyAbstractBlockPropertyFix {
    base: DataFixBase,
    name: String,
    should_fix: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    fix_properties: Arc<dyn Fn(&str, Dynamic) -> Dynamic + Send + Sync>,
}

impl LegacyAbstractBlockPropertyFix {
    pub fn new(
        output_schema: &Arc<Schema>,
        name: &str,
        should_fix: impl Fn(&str) -> bool + Send + Sync + 'static,
        fix_properties: impl Fn(&str, Dynamic) -> Dynamic + Send + Sync + 'static,
    ) -> LegacyAbstractBlockPropertyFix {
        LegacyAbstractBlockPropertyFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), should_fix: Arc::new(should_fix), fix_properties: Arc::new(fix_properties) }
    }
}

impl DataFix for LegacyAbstractBlockPropertyFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let (should_fix, fix_properties) = (self.should_fix.clone(), self.fix_properties.clone());
        self.fix_type_everywhere_typed(&self.name, self.get_input_schema().get_type(BLOCK_STATE), move |input| {
            input.update_remainder(|tag| fix_block_state(&*should_fix, &*fix_properties, tag))
        })
    }
}

fn fix_block_state(should_fix: &dyn Fn(&str) -> bool, fix_properties: &dyn Fn(&str, Dynamic) -> Dynamic, tag: Dynamic) -> Dynamic {
    let block_id = tag.get("Name").as_string().into_result().ok().map(|name| ensure_namespaced(&name));
    match block_id {
        Some(block_id) if should_fix(&block_id) => tag.update("Properties", |properties| fix_properties(&block_id, properties)),
        _ => tag,
    }
}
