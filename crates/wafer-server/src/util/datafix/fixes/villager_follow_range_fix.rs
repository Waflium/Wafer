use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

const ORIGINAL_VALUE: f64 = 16.0;
const NEW_BASE_VALUE: f64 = 48.0;

pub struct VillagerFollowRangeFix;

impl VillagerFollowRangeFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "Villager Follow Range Fix", ENTITY, "minecraft:villager", |entity| entity.update_remainder(fix_value))
    }
}

fn fix_value(tag: Dynamic) -> Dynamic {
    tag.update("Attributes", |attributes| {
        let fixed = attributes.as_stream().into_iter().map(|attribute| {
            if attribute.get("Name").as_string_or("") == "generic.follow_range" && attribute.get("Base").as_double(0.0) == ORIGINAL_VALUE {
                let base = attribute.create_double(NEW_BASE_VALUE);
                attribute.set("Base", base)
            } else {
                attribute
            }
        });
        attributes.create_list(fixed)
    })
}
