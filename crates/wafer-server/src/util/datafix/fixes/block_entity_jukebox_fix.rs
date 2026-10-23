use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field};
use crate::util::datafix::fixes::item_id_fix;
use crate::util::datafix::fixes::item_stack_the_flattening_fix;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityJukeboxFix;

impl BlockEntityJukeboxFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        let input_schema = if changes_type { output_schema.get_parent().clone() } else { output_schema.clone() };
        NamedEntityFix::new(output_schema, changes_type, "BlockEntityJukeboxFix", BLOCK_ENTITY, "minecraft:jukebox", move |entity| fix(&input_schema, entity))
    }
}

fn fix(input_schema: &Schema, entity: Typed) -> Typed {
    let jukebox_type = input_schema.get_choice_type(BLOCK_ENTITY, "minecraft:jukebox");
    let record_item_finder = find_field(&jukebox_type, "RecordItem");
    let item_stack_type = record_item_finder.ty();
    let tag = entity.get_remainder();
    let record_id = tag.get("Record").as_int(0);
    if record_id > 0 {
        if let Some(id) = item_stack_the_flattening_fix::update_item(item_id_fix::get_item(record_id), 0) {
            let item_tag = tag.empty_map();
            let id = item_tag.create_string(id);
            let item_tag = item_tag.set("id", id);
            let count = item_tag.create_byte(1);
            let item_tag = item_tag.set("Count", count);
            let record_item = match Typed::read_typed(&item_stack_type, item_tag).into_result() {
                Ok((record_item, _)) => record_item,
                Err(_) => panic!("Could not create record item stack."),
            };
            return entity.set(&record_item_finder, record_item).set_remainder(tag);
        }
    }
    entity
}
