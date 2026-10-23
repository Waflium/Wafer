use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_poi_section_fix::AbstractPoiSectionFix;

pub struct PoiTypeRenameFix;

impl PoiTypeRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, renamer: impl Fn(&str) -> String + Send + Sync + 'static) -> AbstractPoiSectionFix {
        AbstractPoiSectionFix::new(output_schema, name, move |records| {
            records
                .into_iter()
                .map(|element| {
                    element.update("type", |ty| match ty.as_string().into_result() {
                        Ok(value) => ty.create_string(renamer(&value)),
                        Err(_) => ty,
                    })
                })
                .collect()
        })
    }
}
