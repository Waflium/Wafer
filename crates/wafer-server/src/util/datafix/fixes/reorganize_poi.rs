use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{equals, named, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::POI_CHUNK;

pub struct ReorganizePoi {
    base: DataFixBase,
}

impl ReorganizePoi {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ReorganizePoi {
        ReorganizePoi { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ReorganizePoi {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let poi_chunk_type = named(POI_CHUNK.type_name(), remainder_type());
        if !equals(&poi_chunk_type, &self.get_input_schema().get_type(POI_CHUNK), false) {
            panic!("Poi type is not what was expected.");
        }
        self.fix_type_everywhere("POI reorganization", poi_chunk_type, |input| match input {
            Value::Dynamic(dynamic) => Value::Dynamic(cap(dynamic)),
            other => other,
        })
    }
}

fn cap(mut input: Dynamic) -> Dynamic {
    let mut sections = Vec::new();
    for i in 0..16 {
        let key = i.to_string();
        if let Some(section_records) = input.get(&key).result() {
            let new_section = input.create_map([(input.create_string("Records"), section_records)]);
            sections.push((input.create_string(i.to_string()), new_section));
            input = input.remove(&key);
        }
    }
    let sections = input.create_map(sections);
    input.set("Sections", sections)
}
