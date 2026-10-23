use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{equals, named, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::POI_CHUNK;

pub type ProcessRecords = Arc<dyn Fn(Vec<Dynamic>) -> Vec<Dynamic> + Send + Sync>;

pub struct AbstractPoiSectionFix {
    base: DataFixBase,
    name: String,
    process_records: ProcessRecords,
}

impl AbstractPoiSectionFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, process_records: impl Fn(Vec<Dynamic>) -> Vec<Dynamic> + Send + Sync + 'static) -> AbstractPoiSectionFix {
        AbstractPoiSectionFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), process_records: Arc::new(process_records) }
    }
}

impl DataFix for AbstractPoiSectionFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let poi_chunk_type = named(POI_CHUNK.type_name(), remainder_type());
        if !equals(&poi_chunk_type, &self.get_input_schema().get_type(POI_CHUNK), false) {
            panic!("Poi type is not what was expected.");
        }
        let process_records = self.process_records.clone();
        self.fix_type_everywhere(&self.name, poi_chunk_type, move |input| match input {
            Value::Dynamic(dynamic) => Value::Dynamic(cap(dynamic, &process_records)),
            other => other,
        })
    }
}

fn cap(input: Dynamic, process_records: &ProcessRecords) -> Dynamic {
    input.update("Sections", |sections| sections.update_map_values(|(key, section)| (key, process_section(section, process_records))))
}

fn process_section(section: Dynamic, process_records: &ProcessRecords) -> Dynamic {
    section.update("Records", |records| process_section_records(records, process_records))
}

fn process_section_records(input: Dynamic, process_records: &ProcessRecords) -> Dynamic {
    match input.as_stream_opt().into_result().ok() {
        Some(stream) => input.create_list(process_records(stream)),
        None => input,
    }
}
