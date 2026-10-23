use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct TippedArrowPotionToItemFix;

impl TippedArrowPotionToItemFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix::new(output_schema, false, "TippedArrowPotionToItemFix", ENTITY, "minecraft:arrow", fix)
    }
}

fn fix(input: Dynamic) -> Dynamic {
    let potion = input.get("Potion").result();
    let custom_potion_effects = input.get("custom_potion_effects").result();
    let color = input.get("Color").result();
    if potion.is_none() && custom_potion_effects.is_none() && color.is_none() {
        return input;
    }
    input.remove("Potion").remove("custom_potion_effects").remove("Color").update("item", |item_stack| {
        let mut tag = item_stack.get("tag").or_else_empty_map();
        if let Some(potion) = potion {
            tag = tag.set("Potion", potion);
        }
        if let Some(custom_potion_effects) = custom_potion_effects {
            tag = tag.set("custom_potion_effects", custom_potion_effects);
        }
        if let Some(color) = color {
            tag = tag.set("CustomPotionColor", color);
        }
        item_stack.set("tag", tag)
    })
}
