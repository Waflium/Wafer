use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, and, list, optional_fields};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V4312: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, PLAYER, |s| {
        and(
            ENTITY_EQUIPMENT.in_schema(s),
            optional_fields(vec![
                ("RootVehicle", optional_fields(vec![("Entity", ENTITY_TREE.in_schema(s))])),
                ("ender_pearls", list(ENTITY_TREE.in_schema(s))),
                ("Inventory", list(ITEM_STACK.in_schema(s))),
                ("EnderItems", list(ITEM_STACK.in_schema(s))),
                ("ShoulderEntityLeft", ENTITY_TREE.in_schema(s)),
                ("ShoulderEntityRight", ENTITY_TREE.in_schema(s)),
                ("recipeBook", optional_fields(vec![("recipes", list(RECIPE.in_schema(s))), ("toBeDisplayed", list(RECIPE.in_schema(s)))])),
            ]),
        )
    });
}
