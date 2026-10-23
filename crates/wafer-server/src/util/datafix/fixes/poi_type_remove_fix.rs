use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_poi_section_fix::AbstractPoiSectionFix;

pub struct PoiTypeRemoveFix;

impl PoiTypeRemoveFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, types_to_remove: impl Fn(&str) -> bool + Send + Sync + 'static) -> AbstractPoiSectionFix {
        AbstractPoiSectionFix::new(output_schema, name, move |records| records.into_iter().filter(|record| should_keep_record(record, &types_to_remove)).collect())
    }
}

fn should_keep_record(record: &Dynamic, types_to_remove: &impl Fn(&str) -> bool) -> bool {
    match record.get("type").as_string().into_result() {
        Ok(ty) => !types_to_remove(&ty),
        Err(_) => false,
    }
}
