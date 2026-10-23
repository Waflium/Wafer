use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, compound_list_with_key, int_type, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V2501: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_furnace(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| {
        optional_fields(vec![
            ("Items", list(ITEM_STACK.in_schema(s))),
            ("CustomName", TEXT_COMPONENT.in_schema(s)),
            ("RecipesUsed", compound_list_with_key(RECIPE.in_schema(s), int_type())),
        ])
    });
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register_furnace(&mut map, "minecraft:furnace");
    register_furnace(&mut map, "minecraft:smoker");
    register_furnace(&mut map, "minecraft:blast_furnace");
    map
}
