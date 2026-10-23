use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::ADVANCEMENTS;

pub struct CriteriaRenameFix {
    base: DataFixBase,
    name: String,
    advancement_id: String,
    conversions: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl CriteriaRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, advancement_id: &str, conversions: impl Fn(&str) -> String + Send + Sync + 'static) -> CriteriaRenameFix {
        CriteriaRenameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), advancement_id: advancement_id.to_string(), conversions: Arc::new(conversions) }
    }
}

impl DataFix for CriteriaRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let advancement_id = self.advancement_id.clone();
        let conversions = self.conversions.clone();
        self.fix_type_everywhere_typed(&self.name, self.get_input_schema().get_type(ADVANCEMENTS), move |input| {
            input.update_remainder(|tag| fix_advancements(tag, &advancement_id, &conversions))
        })
    }
}

fn fix_advancements(tag: Dynamic, advancement_id: &str, conversions: &Arc<dyn Fn(&str) -> String + Send + Sync>) -> Dynamic {
    tag.update(advancement_id, |advancement| {
        advancement.update("criteria", |criteria| {
            criteria.update_map_values(|(key, value)| {
                let key = match key.as_string().into_result() {
                    Ok(string) => key.create_string(conversions(&string)),
                    Err(_) => key,
                };
                (key, value)
            })
        })
    })
}
