use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityArmorStandSilentFix;

impl EntityArmorStandSilentFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "EntityArmorStandSilentFix", ENTITY, "ArmorStand", |entity| entity.update_remainder(fix_tag))
    }
}

pub fn fix_tag(input: Dynamic) -> Dynamic {
    if input.get("Silent").as_boolean_or(false) && !input.get("Marker").as_boolean_or(false) { input.remove("Silent") } else { input }
}
