use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LIGHTWEIGHT_LEVEL;

pub struct LevelDatDifficultyFix {
    base: DataFixBase,
}

impl LevelDatDifficultyFix {
    pub fn new(output_schema: &Arc<Schema>) -> LevelDatDifficultyFix {
        LevelDatDifficultyFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for LevelDatDifficultyFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("LevelDatDifficultyFix", self.get_input_schema().get_type(LIGHTWEIGHT_LEVEL), |input| {
            input.update_remainder(|level_data| {
                let difficulty = level_data.get("Difficulty").as_int(2);
                let new_difficulty = match difficulty {
                    0 => "peaceful",
                    1 => "easy",
                    3 => "hard",
                    _ => "normal",
                };
                let difficulty_settings = level_data
                    .empty_map()
                    .set("difficulty", level_data.create_string(new_difficulty))
                    .set("hardcore", level_data.create_boolean(level_data.get("hardcore").as_boolean_or(false)))
                    .set("locked", level_data.create_boolean(level_data.get("DifficultyLocked").as_boolean_or(false)));
                level_data.set("difficulty_settings", difficulty_settings).remove("Difficulty").remove("hardcore").remove("DifficultyLocked")
            })
        })
    }
}
