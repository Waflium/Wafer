use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, compound_list_with_key, const_type, optional_fields, string};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V1125: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register_simple(&mut map, "minecraft:bed");
    map
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, ADVANCEMENTS, |s| {
        optional_fields(vec![
            ("minecraft:adventure/adventuring_time", optional_fields(vec![("criteria", compound_list_with_key(BIOME.in_schema(s), string()))])),
            ("minecraft:adventure/kill_a_mob", optional_fields(vec![("criteria", compound_list_with_key(ENTITY_NAME.in_schema(s), string()))])),
            ("minecraft:adventure/kill_all_mobs", optional_fields(vec![("criteria", compound_list_with_key(ENTITY_NAME.in_schema(s), string()))])),
            ("minecraft:husbandry/bred_all_animals", optional_fields(vec![("criteria", compound_list_with_key(ENTITY_NAME.in_schema(s), string()))])),
        ])
    });
    registry.register_type(false, BIOME, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, ENTITY_NAME, |_| const_type(ConstType::NamespacedString));
}
