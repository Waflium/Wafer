use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::LEVEL;

pub struct LegacyDragonFightFix {
    base: DataFixBase,
}

impl LegacyDragonFightFix {
    pub fn new(output_schema: &Arc<Schema>) -> LegacyDragonFightFix {
        LegacyDragonFightFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn fix_dragon_fight(tag: Dynamic) -> Dynamic {
    tag.update("ExitPortalLocation", extra_data_fix_utils::fix_block_pos)
}

impl DataFix for LegacyDragonFightFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("LegacyDragonFightFix", self.get_input_schema().get_type(LEVEL), |input| {
            input.update_remainder(|tag| {
                let dragon_fight = tag.get("DragonFight");
                if dragon_fight.result().is_some() {
                    return tag;
                }
                let legacy_fight = tag.get("DimensionData").get("1").get("DragonFight").or_else_empty_map();
                let fixed = fix_dragon_fight(legacy_fight);
                tag.set("DragonFight", fixed)
            })
        })
    }
}
