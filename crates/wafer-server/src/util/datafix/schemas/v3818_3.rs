use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields, optional_fields_lazy, or};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3818_3: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

pub fn components() -> TypeTemplates {
    let mut components = TypeTemplates::new();
    register(&mut components, "minecraft:bees", |s| list(optional_fields(vec![("entity_data", ENTITY_TREE.in_schema(s))])));
    register(&mut components, "minecraft:block_entity_data", |s| BLOCK_ENTITY.in_schema(s));
    register(&mut components, "minecraft:bundle_contents", |s| list(ITEM_STACK.in_schema(s)));
    register(&mut components, "minecraft:can_break", |s| {
        optional_fields(vec![("predicates", list(optional_fields(vec![("blocks", or(BLOCK_NAME.in_schema(s), list(BLOCK_NAME.in_schema(s))))])))])
    });
    register(&mut components, "minecraft:can_place_on", |s| {
        optional_fields(vec![("predicates", list(optional_fields(vec![("blocks", or(BLOCK_NAME.in_schema(s), list(BLOCK_NAME.in_schema(s))))])))])
    });
    register(&mut components, "minecraft:charged_projectiles", |s| list(ITEM_STACK.in_schema(s)));
    register(&mut components, "minecraft:container", |s| list(optional_fields(vec![("item", ITEM_STACK.in_schema(s))])));
    register(&mut components, "minecraft:entity_data", |s| ENTITY_TREE.in_schema(s));
    register(&mut components, "minecraft:pot_decorations", |s| list(ITEM_NAME.in_schema(s)));
    register(&mut components, "minecraft:food", |s| optional_fields(vec![("using_converts_to", ITEM_STACK.in_schema(s))]));
    register(&mut components, "minecraft:custom_name", |s| TEXT_COMPONENT.in_schema(s));
    register(&mut components, "minecraft:item_name", |s| TEXT_COMPONENT.in_schema(s));
    register(&mut components, "minecraft:lore", |s| list(TEXT_COMPONENT.in_schema(s)));
    register(&mut components, "minecraft:written_book_content", |s| {
        optional_fields(vec![
            ("pages", list(
                or(
                    optional_fields(vec![("raw", TEXT_COMPONENT.in_schema(s)), ("filtered", TEXT_COMPONENT.in_schema(s))]),
                    TEXT_COMPONENT.in_schema(s),
                ),
            )),
        ])
    });
    components
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, DATA_COMPONENTS, |s| optional_fields_lazy(&components(), s));
}
