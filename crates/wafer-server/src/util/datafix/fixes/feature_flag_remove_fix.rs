use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LIGHTWEIGHT_LEVEL;

pub struct FeatureFlagRemoveFix {
    base: DataFixBase,
    name: String,
    flags_to_remove: Arc<Vec<String>>,
}

impl FeatureFlagRemoveFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, flags_to_remove: &[&str]) -> FeatureFlagRemoveFix {
        FeatureFlagRemoveFix {
            base: DataFixBase::new(output_schema, false),
            name: name.to_string(),
            flags_to_remove: Arc::new(flags_to_remove.iter().map(|flag| flag.to_string()).collect()),
        }
    }
}

impl DataFix for FeatureFlagRemoveFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let flags_to_remove = self.flags_to_remove.clone();
        self.fix_type_everywhere_typed(&self.name, self.get_input_schema().get_type(LIGHTWEIGHT_LEVEL), move |input| input.update_remainder(|tag| fix_tag(tag, &flags_to_remove)))
    }
}

fn fix_tag(tag: Dynamic, flags_to_remove: &[String]) -> Dynamic {
    let mut inactive_features: Vec<Dynamic> = tag.get("removed_features").as_stream();
    let creator = tag.clone();
    let mut result = tag.update("enabled_features", |features| match features.as_stream_opt().into_result().ok() {
        Some(stream) => {
            let kept = stream.into_iter().filter(|feature| match feature.as_string().into_result() {
                Err(_) => true,
                Ok(as_string) => {
                    let should_remove = flags_to_remove.contains(&as_string);
                    if should_remove {
                        inactive_features.push(creator.create_string(as_string));
                    }
                    !should_remove
                }
            });
            creator.create_list(kept)
        }
        None => features,
    });
    if !inactive_features.is_empty() {
        let removed_features = creator.create_list(inactive_features);
        result = result.set("removed_features", removed_features);
    }
    result
}
