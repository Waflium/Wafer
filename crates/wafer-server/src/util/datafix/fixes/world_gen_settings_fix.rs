use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::WORLD_GEN_SETTINGS;
use crate::util::java_integer::parse_int;

static DEFAULTS: [(&str, StructureFeatureConfiguration); 9] = [
    ("minecraft:village", StructureFeatureConfiguration { spacing: 32, separation: 8, salt: 10387312 }),
    ("minecraft:desert_pyramid", StructureFeatureConfiguration { spacing: 32, separation: 8, salt: 14357617 }),
    ("minecraft:igloo", StructureFeatureConfiguration { spacing: 32, separation: 8, salt: 14357618 }),
    ("minecraft:jungle_pyramid", StructureFeatureConfiguration { spacing: 32, separation: 8, salt: 14357619 }),
    ("minecraft:swamp_hut", StructureFeatureConfiguration { spacing: 32, separation: 8, salt: 14357620 }),
    ("minecraft:pillager_outpost", StructureFeatureConfiguration { spacing: 32, separation: 8, salt: 165745296 }),
    ("minecraft:monument", StructureFeatureConfiguration { spacing: 32, separation: 5, salt: 10387313 }),
    ("minecraft:endcity", StructureFeatureConfiguration { spacing: 20, separation: 11, salt: 10387313 }),
    ("minecraft:mansion", StructureFeatureConfiguration { spacing: 80, separation: 20, salt: 10387319 }),
];

pub struct WorldGenSettingsFix {
    base: DataFixBase,
}

impl WorldGenSettingsFix {
    pub fn new(parent: &Arc<Schema>) -> WorldGenSettingsFix {
        WorldGenSettingsFix { base: DataFixBase::new(parent, true) }
    }
}

impl DataFix for WorldGenSettingsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("WorldGenSettings building", self.get_input_schema().get_type(WORLD_GEN_SETTINGS), |settings| settings.update_remainder(fix))
    }
}

fn map(input: &Dynamic, entries: Vec<(&str, Dynamic)>) -> Dynamic {
    input.create_map(entries.into_iter().map(|(key, value)| (input.create_string(key), value)))
}

fn noise(seed: i64, input: &Dynamic, noise_generator_settings: Dynamic, biome_source: Dynamic) -> Dynamic {
    map(input, vec![("type", input.create_string("minecraft:noise")), ("biome_source", biome_source), ("seed", input.create_long(seed)), ("settings", noise_generator_settings)])
}

fn vanilla_biome_source(input: &Dynamic, seed: i64, legacy_biome_init_layer: bool, large_biomes: bool) -> Dynamic {
    let mut entries = vec![
        ("type", input.create_string("minecraft:vanilla_layered")),
        ("seed", input.create_long(seed)),
        ("large_biomes", input.create_boolean(large_biomes)),
    ];
    if legacy_biome_init_layer {
        entries.push(("legacy_biome_init_layer", input.create_boolean(legacy_biome_init_layer)));
    }
    map(input, entries)
}

fn fix(input: Dynamic) -> Dynamic {
    let seed = input.get("RandomSeed").as_long(0);
    let name = input.get("generatorName").as_string().into_result().ok().map(|name| name.to_lowercase());
    let legacy_custom_options = match input.get("legacy_custom_options").as_string().into_result().ok() {
        Some(options) => Some(options),
        None => {
            if name.as_deref() == Some("customized") {
                input.get("generatorOptions").as_string().into_result().ok()
            } else {
                None
            }
        }
    };
    let mut caves = false;
    let generator = match name.as_deref() {
        Some("customized") | None => default_overworld(&input, seed),
        Some("flat") => {
            let flat_settings = input.get("generatorOptions").result();
            let structure_builder = fix_flat_structures(&input, flat_settings.as_ref());
            let layers = flat_settings.as_ref().and_then(|settings| settings.get("layers").result()).unwrap_or_else(|| {
                let bedrock = map(&input, vec![("height", input.create_int(1)), ("block", input.create_string("minecraft:bedrock"))]);
                let dirt = map(&input, vec![("height", input.create_int(2)), ("block", input.create_string("minecraft:dirt"))]);
                let grass_block = map(&input, vec![("height", input.create_int(1)), ("block", input.create_string("minecraft:grass_block"))]);
                input.create_list([bedrock, dirt, grass_block])
            });
            let biome = flat_settings.as_ref().map_or_else(|| "minecraft:plains".to_string(), |settings| settings.get("biome").as_string_or("minecraft:plains"));
            let settings = map(&input, vec![("structures", input.create_map(structure_builder)), ("layers", layers), ("biome", input.create_string(biome))]);
            map(&input, vec![("type", input.create_string("minecraft:flat")), ("settings", settings)])
        }
        Some("debug_all_block_states") => map(&input, vec![("type", input.create_string("minecraft:debug"))]),
        Some("buffet") => {
            let chunk_generator_type = input.get("generatorOptions").get("chunk_generator").get("type").as_string().into_result().ok();
            let noise_generator_settings = if chunk_generator_type.as_deref() == Some("minecraft:caves") {
                caves = true;
                input.create_string("minecraft:caves")
            } else if chunk_generator_type.as_deref() == Some("minecraft:floating_islands") {
                input.create_string("minecraft:floating_islands")
            } else {
                input.create_string("minecraft:overworld")
            };
            let biome_source = input.get("generatorOptions").get("biome_source").result().unwrap_or_else(|| map(&input, vec![("type", input.create_string("minecraft:fixed"))]));
            let fixed_source = if biome_source.get("type").as_string().into_result().ok().as_deref() == Some("minecraft:fixed") {
                let biome = biome_source
                    .get("options")
                    .get("biomes")
                    .as_stream()
                    .into_iter()
                    .next()
                    .and_then(|biome| biome.as_string().into_result().ok())
                    .unwrap_or_else(|| "minecraft:ocean".to_string());
                let biome = input.create_string(biome);
                biome_source.remove("options").set("biome", biome)
            } else {
                biome_source
            };
            noise(seed, &input, noise_generator_settings, fixed_source)
        }
        Some(name) => {
            let normal = name == "default";
            let legacy_biome_init_layer = name == "default_1_1" || normal && input.get("generatorVersion").as_int(0) == 0;
            let is_amplified = name == "amplified";
            let large_biomes = name == "largebiomes";
            noise(
                seed,
                &input,
                input.create_string(if is_amplified { "minecraft:amplified" } else { "minecraft:overworld" }),
                vanilla_biome_source(&input, seed, legacy_biome_init_layer, large_biomes),
            )
        }
    };
    let generate_map_features = input.get("MapFeatures").as_boolean_or(true);
    let generate_bonus_chest = input.get("BonusChest").as_boolean_or(false);
    let mut entries = vec![
        ("seed", input.create_long(seed)),
        ("generate_features", input.create_boolean(generate_map_features)),
        ("bonus_chest", input.create_boolean(generate_bonus_chest)),
        ("dimensions", vanilla_levels(&input, seed, generator, caves)),
    ];
    if let Some(options) = legacy_custom_options {
        entries.push(("legacy_custom_options", input.create_string(options)));
    }
    map(&input, entries)
}

pub fn default_overworld(input: &Dynamic, seed: i64) -> Dynamic {
    noise(seed, input, input.create_string("minecraft:overworld"), vanilla_biome_source(input, seed, false, false))
}

pub fn vanilla_levels(input: &Dynamic, seed: i64, overworld_generator: Dynamic, caves: bool) -> Dynamic {
    let overworld_type = input.create_string(format!("minecraft:overworld{}", if caves { "_caves" } else { "" }));
    let overworld = map(input, vec![("type", overworld_type), ("generator", overworld_generator)]);
    let nether_biome_source = map(
        input,
        vec![("type", input.create_string("minecraft:multi_noise")), ("seed", input.create_long(seed)), ("preset", input.create_string("minecraft:nether"))],
    );
    let nether_generator = noise(seed, input, input.create_string("minecraft:nether"), nether_biome_source);
    let nether = map(input, vec![("type", input.create_string("minecraft:the_nether")), ("generator", nether_generator)]);
    let end_biome_source = map(input, vec![("type", input.create_string("minecraft:the_end")), ("seed", input.create_long(seed))]);
    let end_generator = noise(seed, input, input.create_string("minecraft:end"), end_biome_source);
    let end = map(input, vec![("type", input.create_string("minecraft:the_end")), ("generator", end_generator)]);
    map(input, vec![("minecraft:overworld", overworld), ("minecraft:the_nether", nether), ("minecraft:the_end", end)])
}

fn fix_flat_structures(creator: &Dynamic, settings: Option<&Dynamic>) -> Vec<(Dynamic, Dynamic)> {
    let mut stronghold_distance = 32;
    let mut stronghold_spread = 3;
    let mut stronghold_count = 128;
    let mut has_stronghold = false;
    let mut structure_config: Vec<(&'static str, StructureFeatureConfiguration)> = Vec::new();
    if settings.is_none() {
        has_stronghold = true;
        put_config(&mut structure_config, "minecraft:village", default_configuration("minecraft:village"));
    }
    let structures = settings.and_then(|settings| settings.get("structures").result());
    if let Some(map) = structures.and_then(|structures| structures.get_map_values().into_result().ok()) {
        for (structure_key, value1) in map {
            let Some(options) = value1.get_map_values().into_result().ok() else { continue };
            for (option_key, option_value) in options {
                let structure_name = structure_key.as_string_or("");
                let option_name = option_key.as_string_or("");
                let value = option_value.as_string_or("");
                if structure_name == "stronghold" {
                    has_stronghold = true;
                    match option_name.as_str() {
                        "distance" => stronghold_distance = get_int_min(&value, stronghold_distance, 1),
                        "spread" => stronghold_spread = get_int_min(&value, stronghold_spread, 1),
                        "count" => stronghold_count = get_int_min(&value, stronghold_count, 1),
                        _ => {}
                    }
                } else {
                    match option_name.as_str() {
                        "distance" => match structure_name.as_str() {
                            "village" => set_spacing(&mut structure_config, "minecraft:village", &value, 9),
                            "biome_1" => {
                                set_spacing(&mut structure_config, "minecraft:desert_pyramid", &value, 9);
                                set_spacing(&mut structure_config, "minecraft:igloo", &value, 9);
                                set_spacing(&mut structure_config, "minecraft:jungle_pyramid", &value, 9);
                                set_spacing(&mut structure_config, "minecraft:swamp_hut", &value, 9);
                                set_spacing(&mut structure_config, "minecraft:pillager_outpost", &value, 9);
                            }
                            "endcity" => set_spacing(&mut structure_config, "minecraft:endcity", &value, 1),
                            "mansion" => set_spacing(&mut structure_config, "minecraft:mansion", &value, 1),
                            _ => {}
                        },
                        "separation" => {
                            if structure_name == "oceanmonument" {
                                let config = get_config(&structure_config, "minecraft:monument");
                                let spacing = get_int_min(&value, config.separation, 1);
                                put_config(&mut structure_config, "minecraft:monument", StructureFeatureConfiguration { spacing, separation: config.separation, salt: config.salt });
                            }
                        }
                        "spacing" => {
                            if structure_name == "oceanmonument" {
                                set_spacing(&mut structure_config, "minecraft:monument", &value, 1);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let mut structure_builder = Vec::new();
    let structure_configs = creator.create_map(structure_config.iter().map(|(key, config)| (creator.create_string(*key), config.serialize(creator))));
    structure_builder.push((creator.create_string("structures"), structure_configs));
    if has_stronghold {
        let stronghold = map(
            creator,
            vec![
                ("distance", creator.create_int(stronghold_distance)),
                ("spread", creator.create_int(stronghold_spread)),
                ("count", creator.create_int(stronghold_count)),
            ],
        );
        structure_builder.push((creator.create_string("stronghold"), stronghold));
    }
    structure_builder
}

fn get_int(input: &str, def: i32) -> i32 {
    parse_int(input).unwrap_or(def)
}

fn get_int_min(input: &str, def: i32, min: i32) -> i32 {
    min.max(get_int(input, def))
}

fn default_configuration(structure: &str) -> StructureFeatureConfiguration {
    DEFAULTS.iter().find(|(key, _)| *key == structure).map(|(_, config)| *config).expect("a default structure configuration")
}

fn get_config(structure_config: &[(&'static str, StructureFeatureConfiguration)], structure: &str) -> StructureFeatureConfiguration {
    structure_config.iter().find(|(key, _)| *key == structure).map_or_else(|| default_configuration(structure), |(_, config)| *config)
}

fn put_config(structure_config: &mut Vec<(&'static str, StructureFeatureConfiguration)>, structure: &str, config: StructureFeatureConfiguration) {
    match structure_config.iter_mut().find(|(key, _)| *key == structure) {
        Some((_, slot)) => *slot = config,
        None => {
            let key = DEFAULTS.iter().find(|(key, _)| *key == structure).map(|(key, _)| *key).expect("a default structure configuration");
            structure_config.push((key, config));
        }
    }
}

fn set_spacing(structure_config: &mut Vec<(&'static str, StructureFeatureConfiguration)>, structure: &str, option_value: &str, min: i32) {
    let config = get_config(structure_config, structure);
    let spacing = get_int_min(option_value, config.spacing, min);
    put_config(structure_config, structure, StructureFeatureConfiguration { spacing, separation: config.separation, salt: config.salt });
}

#[derive(Clone, Copy)]
struct StructureFeatureConfiguration {
    spacing: i32,
    separation: i32,
    salt: i32,
}

impl StructureFeatureConfiguration {
    fn serialize(&self, creator: &Dynamic) -> Dynamic {
        map(creator, vec![("spacing", creator.create_int(self.spacing)), ("separation", creator.create_int(self.separation)), ("salt", creator.create_int(self.salt))])
    }
}
