use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;

const WORLD_GEN_SETTINGS: &str = "WorldGenSettings";
const OLD_SETTINGS_KEYS: [&str; 7] = ["RandomSeed", "generatorName", "generatorOptions", "generatorVersion", "legacy_custom_options", "MapFeatures", "BonusChest"];

pub struct LevelLegacyWorldGenSettingsFix {
    base: DataFixBase,
}

impl LevelLegacyWorldGenSettingsFix {
    pub fn new(output_schema: &Arc<Schema>) -> LevelLegacyWorldGenSettingsFix {
        LevelLegacyWorldGenSettingsFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for LevelLegacyWorldGenSettingsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("LevelLegacyWorldGenSettingsFix", self.get_input_schema().get_type(LEVEL), |input| {
            input.update_remainder(|mut data_tag| {
                let mut world_gen_settings = data_tag.get(WORLD_GEN_SETTINGS).or_else_empty_map();
                for key in OLD_SETTINGS_KEYS {
                    if let Some(old_setting) = data_tag.get(key).result() {
                        data_tag = data_tag.remove(key);
                        world_gen_settings = world_gen_settings.set(key, old_setting);
                    }
                }
                data_tag.set(WORLD_GEN_SETTINGS, world_gen_settings)
            })
        })
    }
}
