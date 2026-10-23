use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::CHUNK;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

type Mapper = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub struct RemapChunkStatusFix {
    base: DataFixBase,
    name: String,
    mapper: Mapper,
}

impl RemapChunkStatusFix {
    pub fn new(schema: &Arc<Schema>, name: &str, mapper: impl Fn(&str) -> String + Send + Sync + 'static) -> RemapChunkStatusFix {
        RemapChunkStatusFix { base: DataFixBase::new(schema, false), name: name.to_string(), mapper: Arc::new(mapper) }
    }
}

impl DataFix for RemapChunkStatusFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let mapper = self.mapper.clone();
        self.fix_type_everywhere_typed(&self.name, self.get_input_schema().get_type(CHUNK), move |input| {
            input.update_remainder(|data| {
                data.update("Status", |status| fix_status(status, &mapper))
                    .update("below_zero_retrogen", |below_zero_retrogen| below_zero_retrogen.update("target_status", |status| fix_status(status, &mapper)))
            })
        })
    }
}

fn fix_status(dynamic: Dynamic, mapper: &Mapper) -> Dynamic {
    match dynamic.as_string().into_result() {
        Ok(status) => dynamic.create_string(mapper(&ensure_namespaced(&status))),
        Err(_) => dynamic,
    }
}
