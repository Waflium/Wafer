use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::ADVANCEMENTS;

pub struct AdvancementsRenameFix {
    base: DataFixBase,
    name: String,
    renamer: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl AdvancementsRenameFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool, name: &str, renamer: impl Fn(&str) -> String + Send + Sync + 'static) -> AdvancementsRenameFix {
        AdvancementsRenameFix { base: DataFixBase::new(output_schema, changes_type), name: name.to_string(), renamer: Arc::new(renamer) }
    }
}

impl DataFix for AdvancementsRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let renamer = self.renamer.clone();
        self.fix_type_everywhere_typed(&self.name, self.get_input_schema().get_type(ADVANCEMENTS), move |input| {
            input.update_remainder(|tag| {
                let creator = tag.clone();
                tag.update_map_values(|(key, value)| {
                    let id = key.as_string_or("");
                    (creator.create_string(renamer(&id)), value)
                })
            })
        })
    }
}
