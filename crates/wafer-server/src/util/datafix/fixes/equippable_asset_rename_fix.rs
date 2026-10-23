use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::DATA_COMPONENTS;

pub struct EquippableAssetRenameFix {
    base: DataFixBase,
}

impl EquippableAssetRenameFix {
    pub fn new(output_schema: &Arc<Schema>) -> EquippableAssetRenameFix {
        EquippableAssetRenameFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for EquippableAssetRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let components_type = self.get_input_schema().get_type(DATA_COMPONENTS);
        let equippable_field = find_field(&components_type, "minecraft:equippable");
        self.fix_type_everywhere_typed("equippable asset rename fix", components_type, move |components| {
            components.update_typed(&equippable_field, None, |equippable| equippable.update_remainder(|tag| tag.rename_field("model", "asset_id")))
        })
    }
}
