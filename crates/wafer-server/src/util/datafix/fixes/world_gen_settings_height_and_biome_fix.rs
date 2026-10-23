use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::WORLD_GEN_SETTINGS;

const NAME: &str = "WorldGenSettingsHeightAndBiomeFix";
const WAS_PREVIOUSLY_INCREASED_KEY: &str = "has_increased_height_already";

pub struct WorldGenSettingsHeightAndBiomeFix {
    base: DataFixBase,
}

impl WorldGenSettingsHeightAndBiomeFix {
    pub fn new(output_schema: &Arc<Schema>) -> WorldGenSettingsHeightAndBiomeFix {
        WorldGenSettingsHeightAndBiomeFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for WorldGenSettingsHeightAndBiomeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let world_gen_settings_type = self.get_input_schema().get_type(WORLD_GEN_SETTINGS);
        let dimensions_finder = find_field(&world_gen_settings_type, "dimensions");
        let world_gen_settings_type_new = self.get_output_schema().get_type(WORLD_GEN_SETTINGS);
        let dimensions_type = world_gen_settings_type_new.find_field_type("dimensions").expect("a dimensions field");
        self.fix_type_everywhere_typed_to(NAME, world_gen_settings_type, world_gen_settings_type_new, move |input| {
            let was_increased = input.get_remainder().get(WAS_PREVIOUSLY_INCREASED_KEY).result();
            let was_exp_snap = was_increased.is_none();
            let was_previously_increased = was_increased.is_none_or(|was_increased| was_increased.as_boolean_or(true));
            input.update_remainder(|tag| tag.remove(WAS_PREVIOUSLY_INCREASED_KEY)).update_typed(&dimensions_finder, Some(&dimensions_type), |dimensions| {
                crate::util::write_and_read_typed_or_throw(&dimensions, &dimensions_type, |dimensions_tag| {
                    dimensions_tag.update("minecraft:overworld", |overworld_tag| {
                        overworld_tag.update("generator", |generator| fix_generator(generator, was_exp_snap, was_previously_increased))
                    })
                })
            })
        })
    }
}

fn fix_generator(generator: Dynamic, was_exp_snap: bool, was_previously_increased: bool) -> Dynamic {
    let generator_type = generator.get("type").as_string_or("");
    if generator_type == "minecraft:noise" {
        let mut is_large_biomes = false;
        let generator = generator.update("biome_source", |biome_source| {
            let ty = biome_source.get("type").as_string_or("");
            if ty == "minecraft:vanilla_layered" || was_exp_snap && ty == "minecraft:multi_noise" {
                if biome_source.get("large_biomes").as_boolean_or(false) {
                    is_large_biomes = true;
                }
                let preset_key = biome_source.create_string("preset");
                let preset = biome_source.create_string("minecraft:overworld");
                let type_key = biome_source.create_string("type");
                let multi_noise = biome_source.create_string("minecraft:multi_noise");
                biome_source.create_map([(preset_key, preset), (type_key, multi_noise)])
            } else {
                biome_source
            }
        });
        if is_large_biomes {
            generator.update("settings", |settings| if settings.as_string_or("") == "minecraft:overworld" { settings.create_string("minecraft:large_biomes") } else { settings })
        } else {
            generator
        }
    } else if generator_type == "minecraft:flat" {
        if was_previously_increased { generator } else { generator.update("settings", |settings| settings.update("layers", update_layers)) }
    } else {
        generator
    }
}

fn update_layers(layers: Dynamic) -> Dynamic {
    let height_key = layers.create_string("height");
    let height = layers.create_int(64);
    let block_key = layers.create_string("block");
    let block = layers.create_string("minecraft:air");
    let air_layer = layers.create_map([(height_key, height), (block_key, block)]);
    let all = std::iter::once(air_layer).chain(layers.as_stream());
    layers.create_list(all)
}
