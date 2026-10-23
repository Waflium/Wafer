use std::sync::{Arc, OnceLock};

use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{OpticFinder, Typed, field_finder, find_field};
use crate::datafixers::types::{const_type, named};
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::entity_rename_fix::EntityRenameFix;
use crate::util::datafix::fixes::references::{ENTITY, ITEM_NAME};
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct ThrownPotionSplitFix;

struct ItemIdFinder {
    item_finder: OpticFinder,
    item_id_finder: OpticFinder,
}

impl ItemIdFinder {
    fn get_item_id(&self, entity: &Typed) -> String {
        entity
            .get_optional_typed(&self.item_finder)
            .and_then(|item| item.get_optional(&self.item_id_finder))
            .and_then(|id| id.as_str().map(ensure_namespaced))
            .unwrap_or_default()
    }
}

impl ThrownPotionSplitFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityRenameFix {
        let schema = output_schema.clone();
        let item_id_finder: OnceLock<ItemIdFinder> = OnceLock::new();
        EntityRenameFix::new("ThrownPotionSplitFix", output_schema, true, move |name, entity| {
            if name != "minecraft:potion" {
                return (name.to_string(), entity);
            }
            let item_id_finder = item_id_finder.get_or_init(|| {
                let input_schema = schema.get_parent();
                let potion_type = input_schema.get_choice_type(ENTITY, "minecraft:potion");
                let patched_potion_type = extra_data_fix_utils::patch_sub_type(&potion_type, &input_schema.get_type(ENTITY), &schema.get_type(ENTITY));
                let item_finder = find_field(&patched_potion_type, "Item");
                let item_id_finder = field_finder("id", named(ITEM_NAME.type_name(), const_type(ConstType::NamespacedString)));
                ItemIdFinder { item_finder, item_id_finder }
            });
            let item_id = item_id_finder.get_item_id(&entity);
            if item_id == "minecraft:lingering_potion" { ("minecraft:lingering_potion".to_string(), entity) } else { ("minecraft:splash_potion".to_string(), entity) }
        })
    }
}
