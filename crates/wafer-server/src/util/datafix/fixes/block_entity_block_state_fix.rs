use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, find_field, point};
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct BlockEntityBlockStateFix;

impl BlockEntityBlockStateFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> NamedEntityFix {
        let schema = output_schema.clone();
        NamedEntityFix::new(output_schema, changes_type, "BlockEntityBlockStateFix", BLOCK_ENTITY, "minecraft:piston", move |entity| fix(&schema, entity))
    }
}

fn fix(output_schema: &Schema, entity: Typed) -> Typed {
    let new_type = output_schema.get_choice_type(BLOCK_ENTITY, "minecraft:piston");
    let block_state_finder = find_field(&new_type, "blockState");
    let block_state_type = block_state_finder.ty();
    let tag = entity.get_remainder();
    let block = tag.get("blockId").as_int(0);
    let tag = tag.remove("blockId");
    let data = tag.get("blockData").as_int(0) & 15;
    let tag = tag.remove("blockData");
    let block_state_tag = block_state_data::get_tag(block << 4 | data);
    let output = Typed::new(new_type.clone(), point(&new_type).expect("Could not create new piston block entity."));
    let block_state = match Typed::read_typed(&block_state_type, (*block_state_tag).clone()).into_result() {
        Ok((block_state, _)) => block_state,
        Err(_) => panic!("Could not parse newly created block state tag."),
    };
    output.set_remainder(tag).set(&block_state_finder, block_state)
}
