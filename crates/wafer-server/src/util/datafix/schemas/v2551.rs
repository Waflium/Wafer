use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, compound_list_with_key, const_type, fields, list, optional_fields, or, remainder, string, supplier, tagged_choice_lazy, template_map};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V2551: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, WORLD_GEN_SETTINGS, |_| {
        fields(vec![
            ("dimensions", compound_list_with_key(
                const_type(ConstType::NamespacedString),
                fields(vec![
                    ("generator", tagged_choice_lazy(
                        "type",
                        ConstType::String,
                        template_map(vec![
                            ("minecraft:debug", supplier(|_| remainder())),
                            ("minecraft:flat", supplier(
                                |s| {
                                    optional_fields(vec![
                                        ("settings", optional_fields(vec![
                                            ("biome", BIOME.in_schema(s)),
                                            ("layers", list(optional_fields(vec![("block", BLOCK_NAME.in_schema(s))]))),
                                        ])),
                                    ])
                                },
                            )),
                            ("minecraft:noise", supplier(
                                |s| {
                                    optional_fields(vec![
                                        ("biome_source", tagged_choice_lazy(
                                            "type",
                                            ConstType::String,
                                            template_map(vec![
                                                ("minecraft:fixed", supplier(|s| fields(vec![("biome", BIOME.in_schema(s))]))),
                                                ("minecraft:multi_noise", supplier(|s| list(fields(vec![("biome", BIOME.in_schema(s))])))),
                                                ("minecraft:checkerboard", supplier(|s| fields(vec![("biomes", list(BIOME.in_schema(s)))]))),
                                                ("minecraft:vanilla_layered", supplier(|_| remainder())),
                                                ("minecraft:the_end", supplier(|_| remainder())),
                                            ]),
                                        )),
                                        ("settings", or(
                                            string(),
                                            optional_fields(vec![
                                                ("default_block", BLOCK_NAME.in_schema(s)),
                                                ("default_fluid", BLOCK_NAME.in_schema(s)),
                                            ]),
                                        )),
                                    ])
                                },
                            )),
                        ]),
                    )),
                ]),
            )),
        ])
    });
}
