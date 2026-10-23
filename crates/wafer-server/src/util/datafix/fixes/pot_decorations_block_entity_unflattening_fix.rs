use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::pot_decorations_component_unflattening_fix::unpack_list;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct PotDecorationsBlockEntityUnflatteningFix;

impl PotDecorationsBlockEntityUnflatteningFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        let new_type = output_schema.find_choice_type(BLOCK_ENTITY).choices().get("minecraft:decorated_pot").cloned().expect("a decorated pot type");
        NamedEntityFix::new(output_schema, true, "PotDecorationsBlockEntityUnflatteningFix", BLOCK_ENTITY, "minecraft:decorated_pot", move |entity| {
            crate::util::write_and_read_typed_or_throw(&entity, &new_type, |contents| {
                let original = contents.get("sherds").or_else_empty_list();
                contents.set("sherds", unpack_list(original))
            })
        })
    }
}
