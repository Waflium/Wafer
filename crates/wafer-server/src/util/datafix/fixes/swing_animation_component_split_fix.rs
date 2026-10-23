use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub struct SwingAnimationComponentSplitFix {
    base: DataFixBase,
}

impl SwingAnimationComponentSplitFix {
    pub fn new(output_schema: &Arc<Schema>) -> SwingAnimationComponentSplitFix {
        SwingAnimationComponentSplitFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for SwingAnimationComponentSplitFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("SwingAnimationComponentSplitFix", self.get_input_schema().get_type(DATA_COMPONENTS), |input| {
            input.update_remainder(|tag| match tag.get("minecraft:swing_animation").result() {
                Some(swing_animation) => tag
                    .remove("minecraft:swing_animation")
                    .set("minecraft:attack_animation", swing_animation.clone())
                    .set("minecraft:interact_animation", swing_animation),
                None => tag,
            })
        })
    }
}
