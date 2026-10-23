use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::schemas::schema::Schema;

pub struct WriteAndReadFix {
    base: DataFixBase,
    name: String,
    ty: TypeReference,
}

impl WriteAndReadFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, ty: TypeReference) -> WriteAndReadFix {
        WriteAndReadFix { base: DataFixBase::new(output_schema, true), name: name.to_string(), ty }
    }
}

impl DataFix for WriteAndReadFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.write_and_read(&self.name, self.get_input_schema().get_type(self.ty), self.get_output_schema().get_type(self.ty))
    }
}
