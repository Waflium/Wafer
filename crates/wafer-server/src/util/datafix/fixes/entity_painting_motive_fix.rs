use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

const MAP: [(&str, &str); 3] = [("donkeykong", "donkey_kong"), ("burningskull", "burning_skull"), ("skullandroses", "skull_and_roses")];

pub struct EntityPaintingMotiveFix;

impl EntityPaintingMotiveFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, changes_type, "EntityPaintingMotiveFix", ENTITY, "minecraft:painting", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    match input.get("Motive").as_string().into_result() {
        Ok(motive) => {
            let lower_case_motive = motive.to_lowercase();
            let mapped = MAP.iter().find(|(from, _)| *from == lower_case_motive).map_or(lower_case_motive.as_str(), |(_, to)| to);
            let created = input.create_string(ensure_namespaced(mapped));
            input.set("Motive", created)
        }
        Err(_) => input,
    }
}
