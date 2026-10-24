use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::json_ops;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;
use crate::util::gson;
use crate::util::java_integer::parse_int;

const MAP: &[(&str, &str)] = &[
    ("0", "minecraft:ocean"),
    ("1", "minecraft:plains"),
    ("2", "minecraft:desert"),
    ("3", "minecraft:mountains"),
    ("4", "minecraft:forest"),
    ("5", "minecraft:taiga"),
    ("6", "minecraft:swamp"),
    ("7", "minecraft:river"),
    ("8", "minecraft:nether"),
    ("9", "minecraft:the_end"),
    ("10", "minecraft:frozen_ocean"),
    ("11", "minecraft:frozen_river"),
    ("12", "minecraft:snowy_tundra"),
    ("13", "minecraft:snowy_mountains"),
    ("14", "minecraft:mushroom_fields"),
    ("15", "minecraft:mushroom_field_shore"),
    ("16", "minecraft:beach"),
    ("17", "minecraft:desert_hills"),
    ("18", "minecraft:wooded_hills"),
    ("19", "minecraft:taiga_hills"),
    ("20", "minecraft:mountain_edge"),
    ("21", "minecraft:jungle"),
    ("22", "minecraft:jungle_hills"),
    ("23", "minecraft:jungle_edge"),
    ("24", "minecraft:deep_ocean"),
    ("25", "minecraft:stone_shore"),
    ("26", "minecraft:snowy_beach"),
    ("27", "minecraft:birch_forest"),
    ("28", "minecraft:birch_forest_hills"),
    ("29", "minecraft:dark_forest"),
    ("30", "minecraft:snowy_taiga"),
    ("31", "minecraft:snowy_taiga_hills"),
    ("32", "minecraft:giant_tree_taiga"),
    ("33", "minecraft:giant_tree_taiga_hills"),
    ("34", "minecraft:wooded_mountains"),
    ("35", "minecraft:savanna"),
    ("36", "minecraft:savanna_plateau"),
    ("37", "minecraft:badlands"),
    ("38", "minecraft:wooded_badlands_plateau"),
    ("39", "minecraft:badlands_plateau"),
    ("40", "minecraft:small_end_islands"),
    ("41", "minecraft:end_midlands"),
    ("42", "minecraft:end_highlands"),
    ("43", "minecraft:end_barrens"),
    ("44", "minecraft:warm_ocean"),
    ("45", "minecraft:lukewarm_ocean"),
    ("46", "minecraft:cold_ocean"),
    ("47", "minecraft:deep_warm_ocean"),
    ("48", "minecraft:deep_lukewarm_ocean"),
    ("49", "minecraft:deep_cold_ocean"),
    ("50", "minecraft:deep_frozen_ocean"),
    ("127", "minecraft:the_void"),
    ("129", "minecraft:sunflower_plains"),
    ("130", "minecraft:desert_lakes"),
    ("131", "minecraft:gravelly_mountains"),
    ("132", "minecraft:flower_forest"),
    ("133", "minecraft:taiga_mountains"),
    ("134", "minecraft:swamp_hills"),
    ("140", "minecraft:ice_spikes"),
    ("149", "minecraft:modified_jungle"),
    ("151", "minecraft:modified_jungle_edge"),
    ("155", "minecraft:tall_birch_forest"),
    ("156", "minecraft:tall_birch_hills"),
    ("157", "minecraft:dark_forest_hills"),
    ("158", "minecraft:snowy_taiga_mountains"),
    ("160", "minecraft:giant_spruce_taiga"),
    ("161", "minecraft:giant_spruce_taiga_hills"),
    ("162", "minecraft:modified_gravelly_mountains"),
    ("163", "minecraft:shattered_savanna"),
    ("164", "minecraft:shattered_savanna_plateau"),
    ("165", "minecraft:eroded_badlands"),
    ("166", "minecraft:modified_wooded_badlands_plateau"),
    ("167", "minecraft:modified_badlands_plateau"),
];

pub struct LevelDataGeneratorOptionsFix {
    base: DataFixBase,
}

impl LevelDataGeneratorOptionsFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> LevelDataGeneratorOptionsFix {
        LevelDataGeneratorOptionsFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for LevelDataGeneratorOptionsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let result_type = self.get_output_schema().get_type(LEVEL);
        let target_type = result_type.clone();
        self.fix_type_everywhere_typed_to("LevelDataGeneratorOptionsFix", self.get_input_schema().get_type(LEVEL), result_type, move |input| {
            crate::util::write_and_read_typed_or_throw(&input, &target_type, |tag| {
                let generator_options = tag.get("generatorOptions").as_string().into_result().ok();
                let generator_name = tag.get("generatorName").as_string_or("");
                if generator_name.eq_ignore_ascii_case("flat") {
                    let flat_option_string = generator_options.unwrap_or_default();
                    let converted = convert(&flat_option_string);
                    tag.set("generatorOptions", converted)
                } else if generator_name.eq_ignore_ascii_case("buffet")
                    && let Some(generator_options) = generator_options
                {
                    let legacy_options = gson::parse_lenient(&generator_options).unwrap_or_else(|error| panic!("{error}"));
                    let converted = json_ops::convert_element_to_nbt(&legacy_options).unwrap_or_else(|message| {
                        if message.starts_with("Unsupported null") {
                            panic!("Cannot invoke \"com.google.gson.JsonElement.getAsJsonPrimitive()\" because \"input\" is null");
                        }
                        panic!("{message}")
                    });
                    tag.set("generatorOptions", Dynamic::of(converted))
                } else {
                    tag
                }
            })
        })
    }
}

fn put<V>(entries: &mut Vec<(String, V)>, key: &str, value: V) {
    match entries.iter_mut().find(|(existing, _)| existing == key) {
        Some((_, slot)) => *slot = value,
        None => entries.push((key.to_string(), value)),
    }
}

fn java_split(input: &str, delimiter: char, limit: usize) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut index = 0;
    for (position, character) in input.char_indices() {
        if character == delimiter {
            if limit > 0 && parts.len() == limit - 1 {
                break;
            }
            parts.push(&input[index..position]);
            index = position + character.len_utf8();
        }
    }
    if index == 0 {
        return vec![input];
    }
    parts.push(&input[index..]);
    if limit == 0 {
        while parts.last().is_some_and(|part| part.is_empty()) {
            parts.pop();
        }
    }
    parts
}

fn convert(flat_option_string: &str) -> Dynamic {
    let ops = Dynamic::empty();
    let mut parts = flat_option_string.split(';').peekable();
    let mut biome = "minecraft:plains";
    let mut structures_options: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let layer_list: Vec<(i32, String)>;
    if !flat_option_string.is_empty() && parts.peek().is_some() {
        layer_list = get_layers_info_from_string(parts.next().expect("a layer part"));
        if !layer_list.is_empty() {
            if let Some(part) = parts.next() {
                biome = MAP.iter().find(|(key, _)| *key == part).map_or("minecraft:plains", |(_, value)| value);
            }
            if let Some(part) = parts.next() {
                let structures = part.to_lowercase();
                for structure in java_split(&structures, ',', 0) {
                    let separated = java_split(structure, '(', 2);
                    if !separated[0].is_empty() {
                        put(&mut structures_options, separated[0], Vec::new());
                        if separated.len() > 1 && separated[1].ends_with(')') && separated[1].len() > 1 {
                            let options = &separated[1][..separated[1].len() - 1];
                            for part in java_split(options, ' ', 0) {
                                let split = java_split(part, '=', 2);
                                if split.len() == 2 {
                                    let (_, options) = structures_options.iter_mut().find(|(key, _)| key == separated[0]).expect("structure options");
                                    put(options, split[0], split[1].to_string());
                                }
                            }
                        }
                    }
                }
            } else {
                put(&mut structures_options, "village", Vec::new());
            }
        }
    } else {
        layer_list = vec![(1, "minecraft:bedrock".to_string()), (2, "minecraft:dirt".to_string()), (1, "minecraft:grass_block".to_string())];
        put(&mut structures_options, "village", Vec::new());
    }
    let layers = ops.create_list(layer_list.into_iter().map(|(height, block)| {
        ops.create_map([(ops.create_string("height"), ops.create_int(height)), (ops.create_string("block"), ops.create_string(block))])
    }));
    let structures = ops.create_map(structures_options.into_iter().map(|(structure, options)| {
        (
            ops.create_string(structure.to_lowercase()),
            ops.create_map(options.into_iter().map(|(option, value)| (ops.create_string(option), ops.create_string(value)))),
        )
    }));
    ops.create_map([(ops.create_string("layers"), layers), (ops.create_string("biome"), ops.create_string(biome)), (ops.create_string("structures"), structures)])
}

fn get_layer_info_from_string(input: &str) -> Option<(i32, String)> {
    let parts = java_split(input, '*', 2);
    let height = if parts.len() == 2 {
        match parse_int(parts[0]) {
            Some(height) => height,
            None => return None,
        }
    } else {
        1
    };
    let block = parts[parts.len() - 1];
    Some((height, block.to_string()))
}

fn get_layers_info_from_string(input: &str) -> Vec<(i32, String)> {
    let mut result = Vec::new();
    for depth in java_split(input, ',', 0) {
        match get_layer_info_from_string(depth) {
            Some(layer) => result.push(layer),
            None => return Vec::new(),
        }
    }
    result
}
