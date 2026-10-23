use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct AreaEffectCloudDurationScaleFix;

impl AreaEffectCloudDurationScaleFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "AreaEffectCloudDurationScaleFix", ENTITY, "minecraft:area_effect_cloud", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(|tag| {
        let potion_duration_scale = tag.create_float(0.25);
        tag.set("potion_duration_scale", potion_duration_scale)
    })
}
