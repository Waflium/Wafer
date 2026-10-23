use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::WORLD_GEN_SETTINGS;

pub struct WorldGenSettingsDisallowOldCustomWorldsFix {
    base: DataFixBase,
}

impl WorldGenSettingsDisallowOldCustomWorldsFix {
    pub fn new(output_schema: &Arc<Schema>) -> WorldGenSettingsDisallowOldCustomWorldsFix {
        WorldGenSettingsDisallowOldCustomWorldsFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for WorldGenSettingsDisallowOldCustomWorldsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let world_gen_settings_type = self.get_input_schema().get_type(WORLD_GEN_SETTINGS);
        let dimensions_finder = find_field(&world_gen_settings_type, "dimensions");
        self.fix_type_everywhere_typed(&format!("WorldGenSettingsDisallowOldCustomWorldsFix_{}", self.get_output_schema().version_key), world_gen_settings_type, move |input| {
            input.update_typed(&dimensions_finder, None, |dimensions| {
                if let Some(tag) = dimensions.write().value
                    && let Some(map) = tag.get_map_values().value
                {
                    for (_, value) in map {
                        if value.get("type").as_string().into_result().is_err() {
                            panic!("Unable load old custom worlds.");
                        }
                    }
                }
                dimensions
            })
        })
    }
}
