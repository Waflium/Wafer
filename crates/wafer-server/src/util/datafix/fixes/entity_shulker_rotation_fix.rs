use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityShulkerRotationFix;

impl EntityShulkerRotationFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "EntityShulkerRotationFix", ENTITY, "minecraft:shulker", |entity| entity.update_remainder(fix_tag))
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    let mut rotation: Vec<f64> = input.get("Rotation").as_list(|d| d.as_double(180.0));
    if rotation.is_empty() {
        return input;
    }
    rotation[0] -= 180.0;
    let rotation = input.create_list(rotation.into_iter().map(|value| input.create_double(value)));
    input.set("Rotation", rotation)
}
