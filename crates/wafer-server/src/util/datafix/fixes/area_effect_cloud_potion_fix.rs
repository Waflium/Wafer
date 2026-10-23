use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct AreaEffectCloudPotionFix;

impl AreaEffectCloudPotionFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "AreaEffectCloudPotionFix", ENTITY, "minecraft:area_effect_cloud", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_tag)
}

fn fix_tag(mut entity: Dynamic) -> Dynamic {
    let color = entity.get("Color").result();
    let effects = entity.get("effects").result();
    let potion = entity.get("Potion").result();
    entity = entity.remove("Color").remove("effects").remove("Potion");
    if color.is_none() && effects.is_none() && potion.is_none() {
        return entity;
    }
    let mut potion_contents = entity.empty_map();
    if let Some(color) = color {
        potion_contents = potion_contents.set("custom_color", color);
    }
    if let Some(effects) = effects {
        potion_contents = potion_contents.set("custom_effects", effects);
    }
    if let Some(potion) = potion {
        potion_contents = potion_contents.set("potion", potion);
    }
    entity.set("potion_contents", potion_contents)
}
