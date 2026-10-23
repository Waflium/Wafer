use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, field_finder, type_finder};
use crate::datafixers::types::{TaggedChoiceType, Type, and, and_all, compound_list, const_type, equals, field, list, optional, or, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BIOME, BLOCK_NAME, WORLD_GEN_SETTINGS};
use crate::util::datafix::fixes::world_gen_settings_fix;

pub struct MissingDimensionFix {
    base: DataFixBase,
}

impl MissingDimensionFix {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> MissingDimensionFix {
        MissingDimensionFix { base: DataFixBase::new(schema, changes_type) }
    }
}

fn fields(name: &str, ty: Arc<Type>) -> Arc<Type> {
    and(field(name, ty), remainder_type())
}

fn optional_fields(name: &str, ty: Arc<Type>) -> Arc<Type> {
    and(optional(field(name, ty)), remainder_type())
}

fn optional_fields2(name1: &str, type1: Arc<Type>, name2: &str, type2: Arc<Type>) -> Arc<Type> {
    and_all(vec![optional(field(name1, type1)), optional(field(name2, type2)), remainder_type()])
}

fn tagged_choice_type(types: Vec<(&str, Arc<Type>)>) -> Arc<Type> {
    Arc::new(Type::TaggedChoice(Arc::new(TaggedChoiceType {
        name: "type".into(),
        key_type: ConstType::String,
        types: types.into_iter().map(|(key, ty)| (key.to_string(), ty)).collect(),
    })))
}

impl DataFix for MissingDimensionFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let schema = self.get_input_schema();
        let generator_type = tagged_choice_type(vec![
            ("minecraft:debug", remainder_type()),
            ("minecraft:flat", flat_type(schema)),
            (
                "minecraft:noise",
                optional_fields2(
                    "biome_source",
                    tagged_choice_type(vec![
                        ("minecraft:fixed", fields("biome", schema.get_type(BIOME))),
                        ("minecraft:multi_noise", list(fields("biome", schema.get_type(BIOME)))),
                        ("minecraft:checkerboard", fields("biomes", list(schema.get_type(BIOME)))),
                        ("minecraft:vanilla_layered", remainder_type()),
                        ("minecraft:the_end", remainder_type()),
                    ]),
                    "settings",
                    or(const_type(ConstType::String), optional_fields2("default_block", schema.get_type(BLOCK_NAME), "default_fluid", schema.get_type(BLOCK_NAME))),
                ),
            ),
        ]);
        let dimensions_type = compound_list(const_type(ConstType::NamespacedString), fields("generator", generator_type));
        let expected_dimensions_type = and(dimensions_type.clone(), remainder_type());
        let settings = schema.get_type(WORLD_GEN_SETTINGS);
        let dimensions_finder = field_finder("dimensions", expected_dimensions_type.clone());
        if !settings.find_field_type("dimensions").is_some_and(|ty| equals(&ty, &expected_dimensions_type, false)) {
            panic!("IllegalStateException");
        }
        let dimension_list_finder = type_finder(dimensions_type.clone());
        self.fix_type_everywhere_typed("MissingDimensionFix", settings, move |input| {
            let tag = input.get_remainder();
            input.update_typed(&dimensions_finder, None, |dimensions| {
                dimensions.update_typed(&dimension_list_finder, None, |generators| {
                    let Value::CompoundList(entries) = &generators.value else { panic!("List exptected") };
                    if entries.is_empty() {
                        let new_dimensions = recreate_settings(&tag);
                        match Typed::read_typed(&dimensions_type, new_dimensions).into_result() {
                            Ok((typed, _)) => typed,
                            Err(_) => generators,
                        }
                    } else {
                        generators
                    }
                })
            })
        })
    }
}

fn flat_type(schema: &Schema) -> Arc<Type> {
    optional_fields(
        "settings",
        optional_fields2("biome", schema.get_type(BIOME), "layers", list(optional_fields("block", schema.get_type(BLOCK_NAME)))),
    )
}

fn recreate_settings(tag: &Dynamic) -> Dynamic {
    let seed = tag.get("seed").as_long(0);
    world_gen_settings_fix::vanilla_levels(tag, seed, world_gen_settings_fix::default_overworld(tag, seed), false)
}
