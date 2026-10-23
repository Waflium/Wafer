use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, compound_list_with_key, const_type, hook, int_type, optional_fields, string, tagged_choice_lazy};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register};
use crate::resources::identifier;
use crate::util::datafix::fixes::references::*;

pub static V1451_6: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

pub const SPECIAL_OBJECTIVE_MARKER: &str = "_special";

pub fn unpack_objective_id(input: Dynamic) -> Dynamic {
    let exploded_id = input.get("CriteriaName").as_string().into_result().ok().map(|name| {
        let Some(colon_pos) = name.find(':') else { return ("_special".to_string(), name) };
        let stat_type = identifier::by_separator(&name[..colon_pos], '.');
        let stat_id = identifier::by_separator(&name[colon_pos + 1..], '.');
        match (stat_type, stat_id) {
            (Ok(stat_type), Ok(stat_id)) => (stat_type, stat_id),
            _ => ("_special".to_string(), name),
        }
    });
    match exploded_id {
        Some((first, second)) => {
            let criteria_type = input.create_map([(input.create_string("type"), input.create_string(first)), (input.create_string("id"), input.create_string(second))]);
            input.set("CriteriaType", criteria_type)
        }
        None => input,
    }
}

pub fn repack_objective_id(input: Dynamic) -> Dynamic {
    let repacked_id = input.get("CriteriaType").result().and_then(|ty| {
        let stat_type = ty.get("type").as_string().into_result().ok();
        let stat_id = ty.get("id").as_string().into_result().ok();
        match (stat_type, stat_id) {
            (Some(unpacked_type), Some(stat_id)) => Some(if unpacked_type == "_special" {
                input.create_string(stat_id)
            } else {
                ty.create_string(format!("{}:{}", pack_namespaced_with_dot(&unpacked_type), pack_namespaced_with_dot(&stat_id)))
            }),
            _ => None,
        }
    });
    match repacked_id {
        Some(id) => input.set("CriteriaName", id).remove("CriteriaType"),
        None => input,
    }
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, STATS, |s| {
        optional_fields(vec![
            ("stats", optional_fields(vec![
                ("minecraft:mined", compound_list_with_key(BLOCK_NAME.in_schema(s), int_type())),
                ("minecraft:crafted", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:used", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:broken", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:picked_up", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:dropped", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:killed", compound_list_with_key(ENTITY_NAME.in_schema(s), int_type())),
                ("minecraft:killed_by", compound_list_with_key(ENTITY_NAME.in_schema(s), int_type())),
                ("minecraft:custom", compound_list_with_key(const_type(ConstType::NamespacedString), int_type())),
            ])),
        ])
    });
    let criterion_types = Arc::new(create_criterion_types());
    registry.register_type(false, OBJECTIVE, move |s| {
        hook(
            optional_fields(vec![
                ("CriteriaType", tagged_choice_lazy("type", ConstType::String, criterion_types.clone())),
                ("DisplayName", TEXT_COMPONENT.in_schema(s)),
            ]),
            unpack_objective_id,
            repack_objective_id,
        )
    });
}

pub fn create_criterion_types() -> TypeTemplates {
    let mut criterion_types = TypeTemplates::new();
    register(&mut criterion_types, "minecraft:mined", |s| optional_fields(vec![("id", BLOCK_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:crafted", |s| optional_fields(vec![("id", ITEM_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:used", |s| optional_fields(vec![("id", ITEM_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:broken", |s| optional_fields(vec![("id", ITEM_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:picked_up", |s| optional_fields(vec![("id", ITEM_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:dropped", |s| optional_fields(vec![("id", ITEM_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:killed", |s| optional_fields(vec![("id", ENTITY_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:killed_by", |s| optional_fields(vec![("id", ENTITY_NAME.in_schema(s))]));
    register(&mut criterion_types, "minecraft:custom", |_| optional_fields(vec![("id", const_type(ConstType::NamespacedString))]));
    register(&mut criterion_types, "_special", |_| optional_fields(vec![("id", string())]));
    criterion_types
}

pub fn pack_namespaced_with_dot(location: &str) -> String {
    match identifier::parse(location) {
        Ok(parsed) => parsed.replacen(':', ".", 1),
        Err(_) => location.to_string(),
    }
}
