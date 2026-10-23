use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::BLOCK_STATE;

pub struct CauldronRenameFix {
    base: DataFixBase,
}

impl CauldronRenameFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> CauldronRenameFix {
        CauldronRenameFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for CauldronRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("cauldron_rename_fix", self.get_input_schema().get_type(BLOCK_STATE), |input| input.update_remainder(fix))
    }
}

fn fix(tag: Dynamic) -> Dynamic {
    let name = tag.get("Name").as_string().into_result().ok();
    if name.as_deref() == Some("minecraft:cauldron") {
        let properties = tag.get("Properties").or_else_empty_map();
        if properties.get("level").as_string_or("0") == "0" {
            tag.remove("Properties")
        } else {
            let water_cauldron = tag.create_string("minecraft:water_cauldron");
            tag.set("Name", water_cauldron)
        }
    } else {
        tag
    }
}
