use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplate, TypeTemplates, and, list, optional_fields, optional_fields_with_rest, tagged_choice_lazy};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V1458: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    let entities = entity_types.clone();
    registry.register_type(true, ENTITY, move |s| {
        and(
            ENTITY_EQUIPMENT.in_schema(s),
            optional_fields_with_rest(
                vec![("CustomName", TEXT_COMPONENT.in_schema(s))],
                tagged_choice_lazy("id", ConstType::NamespacedString, entities.clone()),
            ),
        )
    });
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:beacon", |s| nameable(s));
    register(&mut map, "minecraft:banner", |s| nameable(s));
    register(&mut map, "minecraft:brewing_stand", |s| nameable_inventory(s));
    register(&mut map, "minecraft:chest", |s| nameable_inventory(s));
    register(&mut map, "minecraft:trapped_chest", |s| nameable_inventory(s));
    register(&mut map, "minecraft:dispenser", |s| nameable_inventory(s));
    register(&mut map, "minecraft:dropper", |s| nameable_inventory(s));
    register(&mut map, "minecraft:enchanting_table", |s| nameable(s));
    register(&mut map, "minecraft:furnace", |s| nameable_inventory(s));
    register(&mut map, "minecraft:hopper", |s| nameable_inventory(s));
    register(&mut map, "minecraft:shulker_box", |s| nameable_inventory(s));
    map
}

pub fn nameable_inventory(schema: &Schema) -> TypeTemplate {
    optional_fields(vec![("Items", list(ITEM_STACK.in_schema(schema))), ("CustomName", TEXT_COMPONENT.in_schema(schema))])
}

pub fn nameable(schema: &Schema) -> TypeTemplate {
    optional_fields(vec![("CustomName", TEXT_COMPONENT.in_schema(schema))])
}
