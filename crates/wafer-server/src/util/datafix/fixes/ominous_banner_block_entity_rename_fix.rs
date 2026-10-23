use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, TEXT_COMPONENT};

pub struct OminousBannerBlockEntityRenameFix;

impl OminousBannerBlockEntityRenameFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        let input_schema = if changes_type { output_schema.get_parent().clone() } else { output_schema.clone() };
        let text_component_type = input_schema.get_type(TEXT_COMPONENT);
        NamedEntityFix::new(output_schema, changes_type, "OminousBannerBlockEntityRenameFix", BLOCK_ENTITY, "minecraft:banner", move |entity| {
            let custom_name_f = find_field(&entity.ty, "CustomName");
            let text_component_f = type_finder(text_component_type.clone());
            entity.update_typed(&custom_name_f, None, |custom_name| {
                custom_name.update(&text_component_f, |text_component| {
                    text_component.map_string(|name| name.replace("\"translate\":\"block.minecraft.illager_banner\"", "\"translate\":\"block.minecraft.ominous_banner\""))
                })
            })
        })
    }
}
