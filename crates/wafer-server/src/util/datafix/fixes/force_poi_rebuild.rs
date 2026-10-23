use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{equals, named, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::POI_CHUNK;

pub struct ForcePoiRebuild {
    base: DataFixBase,
}

impl ForcePoiRebuild {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ForcePoiRebuild {
        ForcePoiRebuild { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ForcePoiRebuild {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let poi_chunk_type = named(POI_CHUNK.type_name(), remainder_type());
        if !equals(&poi_chunk_type, &self.get_input_schema().get_type(POI_CHUNK), false) {
            panic!("Poi type is not what was expected.");
        }
        self.fix_type_everywhere("POI rebuild", poi_chunk_type, |input| match input {
            Value::Dynamic(dynamic) => Value::Dynamic(cap(dynamic)),
            other => other,
        })
    }
}

fn cap(input: Dynamic) -> Dynamic {
    input.update("Sections", |sections| sections.update_map_values(|(key, section)| (key, section.remove("Valid"))))
}
