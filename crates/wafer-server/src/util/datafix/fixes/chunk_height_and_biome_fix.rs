use std::collections::HashSet;
use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::chunk_proto_tick_list_fix::PoorMansPalettedContainer;
use crate::util::datafix::fixes::references::CHUNK;
use crate::util::datafix::fixes::world_gen_settings_fix::java_parse_int;

const OLD_SECTION_COUNT: usize = 16;
const NEW_SECTION_COUNT: usize = 24;
const NEW_MIN_SECTION_Y: i32 = -4;
const HEIGHTMAP_BITS: i32 = 9;
const HEIGHTMAP_MASK: i64 = 511;
const HEIGHTMAP_OFFSET: i64 = 64;
const HEIGHTMAP_TYPES: [&str; 7] = ["WORLD_SURFACE_WG", "WORLD_SURFACE", "WORLD_SURFACE_IGNORE_SNOW", "OCEAN_FLOOR_WG", "OCEAN_FLOOR", "MOTION_BLOCKING", "MOTION_BLOCKING_NO_LEAVES"];
const STATUS_IS_OR_AFTER_SURFACE: [&str; 8] = ["surface", "carvers", "liquid_carvers", "features", "light", "spawn", "heightmaps", "full"];
const STATUS_IS_OR_AFTER_NOISE: [&str; 9] = ["noise", "surface", "carvers", "liquid_carvers", "features", "light", "spawn", "heightmaps", "full"];
const BLOCKS_BEFORE_FEATURE_STATUS: [&str; 34] = [
    "minecraft:air",
    "minecraft:basalt",
    "minecraft:bedrock",
    "minecraft:blackstone",
    "minecraft:calcite",
    "minecraft:cave_air",
    "minecraft:coarse_dirt",
    "minecraft:crimson_nylium",
    "minecraft:dirt",
    "minecraft:end_stone",
    "minecraft:grass_block",
    "minecraft:gravel",
    "minecraft:ice",
    "minecraft:lava",
    "minecraft:mycelium",
    "minecraft:nether_wart_block",
    "minecraft:netherrack",
    "minecraft:orange_terracotta",
    "minecraft:packed_ice",
    "minecraft:podzol",
    "minecraft:powder_snow",
    "minecraft:red_sand",
    "minecraft:red_sandstone",
    "minecraft:sand",
    "minecraft:sandstone",
    "minecraft:snow_block",
    "minecraft:soul_sand",
    "minecraft:soul_soil",
    "minecraft:stone",
    "minecraft:terracotta",
    "minecraft:warped_nylium",
    "minecraft:warped_wart_block",
    "minecraft:water",
    "minecraft:white_terracotta",
];
const BIOME_CONTAINER_LAYER_SIZE: usize = 16;
const BIOME_CONTAINER_SIZE: usize = 64;
const BIOME_CONTAINER_TOP_LAYER_OFFSET: usize = 1008;

pub struct ChunkHeightAndBiomeFix {
    base: DataFixBase,
}

impl ChunkHeightAndBiomeFix {
    pub fn new(output_schema: &Arc<Schema>) -> ChunkHeightAndBiomeFix {
        ChunkHeightAndBiomeFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for ChunkHeightAndBiomeFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_chunk_type = self.get_input_schema().get_type(CHUNK);
        let level_finder = find_field(&old_chunk_type, "Level");
        let sections_finder = find_field(&level_finder.ty(), "Sections");
        let chunk_type = self.get_output_schema().get_type(CHUNK);
        let level_type = find_field(&chunk_type, "Level").ty();
        let sections_type = find_field(&level_type, "Sections").ty();
        self.fix_type_everywhere_typed_to("ChunkHeightAndBiomeFix", old_chunk_type, chunk_type, move |chunk| {
            let context_remainder = chunk.get_remainder();
            let dimension = context_remainder.get("__context").get("dimension").as_string().into_result().unwrap_or_default();
            let generator = context_remainder.get("__context").get("generator").as_string().into_result().unwrap_or_default();
            chunk.update_typed(&level_finder, Some(&level_type), |level| {
                let tag = level.get_remainder();
                let is_overworld = dimension == "minecraft:overworld";
                let mut was_increased_height = false;
                let min_section = if is_overworld { -4 } else { 0 };
                let biome_containers = get_biome_containers(&tag, is_overworld, min_section, &mut was_increased_height);
                let air_name = tag.create_string("minecraft:air");
                let air_state = tag.create_map([(tag.create_string("Name"), air_name)]);
                let air_container = make_paletted_container(tag.create_list([air_state]));
                let mut blocks_in_chunk: HashSet<String> = HashSet::new();
                let mut bedrock_section_blocks: Option<PoorMansPalettedContainer> = None;
                let level = level.update_typed(&sections_finder, Some(&sections_type), |sections| {
                    let mut done_sections: HashSet<i32> = HashSet::new();
                    let dynamic = sections.write().into_result().unwrap_or_else(|_| panic!("Malformed Chunk.Level.Sections"));
                    let mut sections_list: Vec<Dynamic> = dynamic
                        .as_stream()
                        .into_iter()
                        .map(|section| {
                            let section_y = section.get("Y").as_int(0);
                            let block_states_container = match section.get("Palette").result().and_then(|palette| {
                                for block_state in palette.as_stream() {
                                    blocks_in_chunk.insert(block_state.get("Name").as_string_or("minecraft:air"));
                                }
                                section.get("BlockStates").result().map(|block_states| make_optimized_paletted_container(palette, block_states))
                            }) {
                                Some(container) => container,
                                None => air_container.clone(),
                            };
                            let is_bedrock_section = section.get("Y").as_int(i32::MAX) == 0;
                            let mut result = section;
                            let section_y_index = section_y - min_section;
                            if section_y_index >= 0 && (section_y_index as usize) < biome_containers.len() {
                                result = result.set("biomes", biome_containers[section_y_index as usize].clone());
                            }
                            done_sections.insert(section_y);
                            if is_bedrock_section {
                                let palette = block_states_container.get("palette").as_list(|dynamic| dynamic);
                                let data = block_states_container.get("data").as_long_stream();
                                bedrock_section_blocks = Some(PoorMansPalettedContainer::new(palette, data));
                            }
                            result.set("block_states", block_states_container).remove("Palette").remove("BlockStates")
                        })
                        .collect();
                    for section_index in 0..biome_containers.len() {
                        let section_y = section_index as i32 + min_section;
                        if done_sections.insert(section_y) {
                            let section_y_tag = tag.create_int(section_y);
                            let section = tag.create_map([(tag.create_string("Y"), section_y_tag)]);
                            let section = section.set("block_states", air_container.clone());
                            let section = section.set("biomes", biome_containers[section_index].clone());
                            sections_list.push(section);
                        }
                    }
                    crate::util::read_typed_or_throw(&sections_type, tag.create_list(sections_list), false)
                });
                level.update_remainder(|chunk_tag| {
                    let chunk_tag = if is_overworld { predict_chunk_status_before_surface(chunk_tag, &mut blocks_in_chunk) } else { chunk_tag };
                    update_chunk_tag(chunk_tag, is_overworld, was_increased_height, generator == "minecraft:noise", bedrock_section_blocks.as_ref())
                })
            })
        })
    }
}

fn predict_chunk_status_before_surface(chunk_tag: Dynamic, blocks_in_chunk: &mut HashSet<String>) -> Dynamic {
    chunk_tag.update("Status", |status_dynamic| {
        let status = status_dynamic.as_string_or("empty");
        if STATUS_IS_OR_AFTER_SURFACE.contains(&status.as_str()) {
            return status_dynamic;
        }
        blocks_in_chunk.remove("minecraft:air");
        let has_non_air_blocks = !blocks_in_chunk.is_empty();
        blocks_in_chunk.retain(|block| !BLOCKS_BEFORE_FEATURE_STATUS.contains(&block.as_str()));
        let has_feature_blocks = !blocks_in_chunk.is_empty();
        if has_feature_blocks {
            status_dynamic.create_string("liquid_carvers")
        } else if status == "noise" || has_non_air_blocks {
            status_dynamic.create_string("noise")
        } else if status == "biomes" {
            status_dynamic.create_string("structure_references")
        } else {
            status_dynamic
        }
    })
}

fn get_biome_containers(tag: &Dynamic, increase_height: bool, min_section: i32, was_increased_height: &mut bool) -> Vec<Dynamic> {
    let mut biome_containers: Vec<Option<Dynamic>> = vec![None; if increase_height { NEW_SECTION_COUNT } else { OLD_SECTION_COUNT }];
    let old_biomes = tag.get("Biomes").result().and_then(|biomes| biomes.as_int_stream_opt().into_result().ok());
    match old_biomes {
        Some(old_biomes) if old_biomes.len() == 1536 => {
            *was_increased_height = true;
            for section_y_index in 0..NEW_SECTION_COUNT {
                biome_containers[section_y_index] = Some(make_biome_container(tag, |ix| get_old_biome(&old_biomes, section_y_index * BIOME_CONTAINER_SIZE + ix)));
            }
        }
        Some(old_biomes) if old_biomes.len() == 1024 => {
            for section_y in 0..OLD_SECTION_COUNT {
                let section_y_index = section_y as i32 - min_section;
                biome_containers[section_y_index as usize] = Some(make_biome_container(tag, |ix| get_old_biome(&old_biomes, section_y * BIOME_CONTAINER_SIZE + ix)));
            }
            if increase_height {
                let below_world_biomes = make_biome_container(tag, |ix| get_old_biome(&old_biomes, ix % BIOME_CONTAINER_LAYER_SIZE));
                let above_world_biomes = make_biome_container(tag, |ix| get_old_biome(&old_biomes, ix % BIOME_CONTAINER_LAYER_SIZE + BIOME_CONTAINER_TOP_LAYER_OFFSET));
                for container in biome_containers.iter_mut().take(4) {
                    *container = Some(below_world_biomes.clone());
                }
                for container in biome_containers.iter_mut().take(24).skip(20) {
                    *container = Some(above_world_biomes.clone());
                }
            }
        }
        _ => {
            let plains = tag.create_string("minecraft:plains");
            let plains_container = make_paletted_container(tag.create_list([plains]));
            biome_containers.fill(Some(plains_container));
        }
    }
    biome_containers.into_iter().map(|container| container.expect("a biome container")).collect()
}

fn get_old_biome(old_biomes: &[i32], index: usize) -> i32 {
    old_biomes[index] & 0xFF
}

fn update_chunk_tag(
    chunk_tag: Dynamic,
    is_overworld: bool,
    was_increased_height: bool,
    needs_blending_and_upgrade: bool,
    bedrock_section_blocks: Option<&PoorMansPalettedContainer>,
) -> Dynamic {
    let mut chunk_tag = chunk_tag.remove("Biomes");
    if !is_overworld {
        return update_carving_masks(chunk_tag, 16, 0);
    }
    if was_increased_height {
        return update_carving_masks(chunk_tag, 24, 0);
    }
    chunk_tag = update_heightmaps(chunk_tag);
    chunk_tag = add_padding_entries(chunk_tag, "LiquidsToBeTicked");
    chunk_tag = add_padding_entries(chunk_tag, "PostProcessing");
    chunk_tag = add_padding_entries(chunk_tag, "ToBeTicked");
    chunk_tag = update_carving_masks(chunk_tag, 24, 4);
    chunk_tag = chunk_tag.update("UpgradeData", shift_upgrade_data);
    if !needs_blending_and_upgrade {
        return chunk_tag;
    }
    let Some(status) = chunk_tag.get("Status").result() else { return chunk_tag };
    let last_status = status.as_string_or("");
    if last_status != "empty" {
        let old_noise_key = chunk_tag.create_string("old_noise");
        let old_noise = chunk_tag.create_boolean(STATUS_IS_OR_AFTER_NOISE.contains(&last_status.as_str()));
        let blending_data = chunk_tag.create_map([(old_noise_key, old_noise)]);
        chunk_tag = chunk_tag.set("blending_data", blending_data);
        if let Some(poor_mans_paletted_container) = bedrock_section_blocks {
            let mut missing_bedrock = [0u64; 4];
            let mut has_any_bedrock = last_status == "noise";
            for z in 0..16 {
                for x in 0..16 {
                    let block_state = poor_mans_paletted_container.get(x, 0, z);
                    let is_bedrock = block_state.is_some_and(|block_state| block_state.get("Name").as_string_or("") == "minecraft:bedrock");
                    let is_air = block_state.is_some_and(|block_state| block_state.get("Name").as_string_or("") == "minecraft:air");
                    if is_air {
                        let bit = (z * 16 + x) as usize;
                        missing_bedrock[bit >> 6] |= 1 << (bit & 63);
                    }
                    has_any_bedrock |= is_bedrock;
                }
            }
            let cardinality: u32 = missing_bedrock.iter().map(|word| word.count_ones()).sum();
            if has_any_bedrock && cardinality != 256 {
                let target_status = if last_status == "full" { chunk_tag.create_string("heightmaps") } else { status };
                let words_in_use = missing_bedrock.iter().rposition(|word| *word != 0).map_or(0, |index| index + 1);
                let target_status_key = chunk_tag.create_string("target_status");
                let missing_bedrock_key = chunk_tag.create_string("missing_bedrock");
                let missing_bedrock_list = chunk_tag.create_long_list(missing_bedrock[..words_in_use].iter().map(|word| *word as i64));
                let below_zero_retrogen = chunk_tag.create_map([(target_status_key, target_status), (missing_bedrock_key, missing_bedrock_list)]);
                chunk_tag = chunk_tag.set("below_zero_retrogen", below_zero_retrogen);
                let empty = chunk_tag.create_string("empty");
                chunk_tag = chunk_tag.set("Status", empty);
            }
            let is_light_on = chunk_tag.create_boolean(false);
            chunk_tag = chunk_tag.set("isLightOn", is_light_on);
        }
    }
    chunk_tag
}

fn shift_upgrade_data(upgrade_data: Dynamic) -> Dynamic {
    upgrade_data.update("Indices", |indices| {
        let mut shifted_indices: Vec<(Dynamic, Dynamic)> = Vec::new();
        if let Ok(entries) = indices.get_map_values().into_result() {
            for (index, data) in entries {
                if let Some(Some(parsed)) = index.as_string().into_result().ok().map(|index| java_parse_int(&index)) {
                    let shifted_index = parsed.wrapping_sub(NEW_MIN_SECTION_Y);
                    let key = index.create_string(shifted_index.to_string());
                    match shifted_indices.iter_mut().find(|(existing, _)| *existing == key) {
                        Some((_, slot)) => *slot = data,
                        None => shifted_indices.push((key, data)),
                    }
                }
            }
        }
        indices.create_map(shifted_indices)
    })
}

fn update_carving_masks(chunk_tag: Dynamic, section_count: usize, added_sections_below: usize) -> Dynamic {
    let carving_masks = chunk_tag.get("CarvingMasks").or_else_empty_map();
    let carving_masks = carving_masks.update_map_values(|(key, value)| {
        let bytes: Vec<u8> = value.as_byte_buffer_opt().into_result().ok().unwrap_or_default().into_iter().map(|byte| byte as u8).collect();
        let mut old_values: Vec<i64> = bytes
            .chunks(8)
            .map(|chunk| {
                let mut word = [0u8; 8];
                word[..chunk.len()].copy_from_slice(chunk);
                i64::from_le_bytes(word)
            })
            .collect();
        while old_values.last() == Some(&0) {
            old_values.pop();
        }
        let mut new_values = vec![0i64; 64 * section_count];
        new_values[64 * added_sections_below..64 * added_sections_below + old_values.len()].copy_from_slice(&old_values);
        (key, chunk_tag.create_long_list(new_values))
    });
    chunk_tag.set("CarvingMasks", carving_masks)
}

fn add_padding_entries(chunk_tag: Dynamic, key: &str) -> Dynamic {
    let mut list = chunk_tag.get(key).or_else_empty_list().as_stream();
    if list.len() == 24 {
        return chunk_tag;
    }
    let empty_list = chunk_tag.empty_list();
    for _ in 0..4 {
        list.insert(0, empty_list.clone());
        list.push(empty_list.clone());
    }
    let list = chunk_tag.create_list(list);
    chunk_tag.set(key, list)
}

fn update_heightmaps(chunk_tag: Dynamic) -> Dynamic {
    chunk_tag.update("Heightmaps", |mut heightmap_tag| {
        for heightmap_type in HEIGHTMAP_TYPES {
            heightmap_tag = heightmap_tag.update(heightmap_type, get_fixed_heightmap);
        }
        heightmap_tag
    })
}

fn get_fixed_heightmap(tag: Dynamic) -> Dynamic {
    let values: Vec<i64> = tag
        .as_long_stream()
        .into_iter()
        .map(|value| {
            let mut new_value = 0i64;
            let mut bit_index = 0;
            while bit_index + HEIGHTMAP_BITS <= 64 {
                let old_height = value >> bit_index & HEIGHTMAP_MASK;
                let new_height = if old_height == 0 { 0 } else { (old_height + HEIGHTMAP_OFFSET).min(HEIGHTMAP_MASK) };
                new_value |= new_height << bit_index;
                bit_index += HEIGHTMAP_BITS;
            }
            new_value
        })
        .collect();
    tag.create_long_list(values)
}

fn make_biome_container(tag: &Dynamic, source_storage: impl Fn(usize) -> i32) -> Dynamic {
    let mut id_map: Vec<i32> = Vec::new();
    for i in 0..BIOME_CONTAINER_SIZE {
        let biome_id = source_storage(i);
        if !id_map.contains(&biome_id) {
            id_map.push(biome_id);
        }
    }
    let palette = tag.create_list(id_map.iter().map(|biome_id| tag.create_string(biome_by_id(*biome_id))));
    let bits = ceillog2(id_map.len() as i32);
    if bits == 0 {
        return make_paletted_container(palette);
    }
    let values_per_long = 64 / bits;
    let required_length = (64 + values_per_long - 1) / values_per_long;
    let mut bit_storage = vec![0i64; required_length as usize];
    let mut cell_index = 0;
    let mut bit_index = 0;
    for i in 0..BIOME_CONTAINER_SIZE {
        let biome_id = source_storage(i);
        let palette_index = id_map.iter().position(|id| *id == biome_id).expect("a mapped biome") as i64;
        bit_storage[cell_index] |= palette_index << bit_index;
        bit_index += bits;
        if bit_index + bits > 64 {
            cell_index += 1;
            bit_index = 0;
        }
    }
    let storage = tag.create_long_list(bit_storage);
    make_paletted_container_with_storage(palette, storage)
}

fn make_paletted_container(palette: Dynamic) -> Dynamic {
    let key = palette.create_string("palette");
    palette.create_map([(key, palette.clone())])
}

fn make_paletted_container_with_storage(palette: Dynamic, storage: Dynamic) -> Dynamic {
    let palette_key = palette.create_string("palette");
    let data_key = palette.create_string("data");
    palette.create_map([(palette_key, palette.clone()), (data_key, storage)])
}

fn make_optimized_paletted_container(palette: Dynamic, data: Dynamic) -> Dynamic {
    let mut palette_list = palette.as_stream();
    if palette_list.len() == 1 {
        return make_paletted_container(palette);
    }
    let palette = pad_palette_entries(palette, &data, &mut palette_list);
    make_paletted_container_with_storage(palette, data)
}

fn pad_palette_entries(palette: Dynamic, data: &Dynamic, palette_list: &mut Vec<Dynamic>) -> Dynamic {
    let data_size_in_bits = data.as_long_stream().len() as i64 * 64;
    let estimated_bits_per_block = data_size_in_bits / 4096;
    let palette_size = palette_list.len() as i32;
    let expected_bits_per_block = ceillog2(palette_size);
    if estimated_bits_per_block <= expected_bits_per_block as i64 {
        return palette;
    }
    let name_key = palette.create_string("Name");
    let name = palette.create_string("minecraft:air");
    let air_pallete_entry = palette.create_map([(name_key, name)]);
    let minimum_palette_size_to_match_data = (1i32.wrapping_shl((estimated_bits_per_block - 1) as i32 as u32)).wrapping_add(1);
    let additional_palette_entries = minimum_palette_size_to_match_data.wrapping_sub(palette_size);
    for _ in 0..additional_palette_entries {
        palette_list.push(air_pallete_entry.clone());
    }
    palette.create_list(std::mem::take(palette_list))
}

pub fn ceillog2(input: i32) -> i32 {
    if input == 0 { 0 } else { ((input as f64).ln() / 2.0f64.ln()).ceil() as i32 }
}

fn biome_by_id(id: i32) -> &'static str {
    match id {
        0 => "minecraft:ocean",
        1 => "minecraft:plains",
        2 => "minecraft:desert",
        3 => "minecraft:mountains",
        4 => "minecraft:forest",
        5 => "minecraft:taiga",
        6 => "minecraft:swamp",
        7 => "minecraft:river",
        8 => "minecraft:nether_wastes",
        9 => "minecraft:the_end",
        10 => "minecraft:frozen_ocean",
        11 => "minecraft:frozen_river",
        12 => "minecraft:snowy_tundra",
        13 => "minecraft:snowy_mountains",
        14 => "minecraft:mushroom_fields",
        15 => "minecraft:mushroom_field_shore",
        16 => "minecraft:beach",
        17 => "minecraft:desert_hills",
        18 => "minecraft:wooded_hills",
        19 => "minecraft:taiga_hills",
        20 => "minecraft:mountain_edge",
        21 => "minecraft:jungle",
        22 => "minecraft:jungle_hills",
        23 => "minecraft:jungle_edge",
        24 => "minecraft:deep_ocean",
        25 => "minecraft:stone_shore",
        26 => "minecraft:snowy_beach",
        27 => "minecraft:birch_forest",
        28 => "minecraft:birch_forest_hills",
        29 => "minecraft:dark_forest",
        30 => "minecraft:snowy_taiga",
        31 => "minecraft:snowy_taiga_hills",
        32 => "minecraft:giant_tree_taiga",
        33 => "minecraft:giant_tree_taiga_hills",
        34 => "minecraft:wooded_mountains",
        35 => "minecraft:savanna",
        36 => "minecraft:savanna_plateau",
        37 => "minecraft:badlands",
        38 => "minecraft:wooded_badlands_plateau",
        39 => "minecraft:badlands_plateau",
        40 => "minecraft:small_end_islands",
        41 => "minecraft:end_midlands",
        42 => "minecraft:end_highlands",
        43 => "minecraft:end_barrens",
        44 => "minecraft:warm_ocean",
        45 => "minecraft:lukewarm_ocean",
        46 => "minecraft:cold_ocean",
        47 => "minecraft:deep_warm_ocean",
        48 => "minecraft:deep_lukewarm_ocean",
        49 => "minecraft:deep_cold_ocean",
        50 => "minecraft:deep_frozen_ocean",
        127 => "minecraft:the_void",
        129 => "minecraft:sunflower_plains",
        130 => "minecraft:desert_lakes",
        131 => "minecraft:gravelly_mountains",
        132 => "minecraft:flower_forest",
        133 => "minecraft:taiga_mountains",
        134 => "minecraft:swamp_hills",
        140 => "minecraft:ice_spikes",
        149 => "minecraft:modified_jungle",
        151 => "minecraft:modified_jungle_edge",
        155 => "minecraft:tall_birch_forest",
        156 => "minecraft:tall_birch_hills",
        157 => "minecraft:dark_forest_hills",
        158 => "minecraft:snowy_taiga_mountains",
        160 => "minecraft:giant_spruce_taiga",
        161 => "minecraft:giant_spruce_taiga_hills",
        162 => "minecraft:modified_gravelly_mountains",
        163 => "minecraft:shattered_savanna",
        164 => "minecraft:shattered_savanna_plateau",
        165 => "minecraft:eroded_badlands",
        166 => "minecraft:modified_wooded_badlands_plateau",
        167 => "minecraft:modified_badlands_plateau",
        168 => "minecraft:bamboo_jungle",
        169 => "minecraft:bamboo_jungle_hills",
        170 => "minecraft:soul_sand_valley",
        171 => "minecraft:crimson_forest",
        172 => "minecraft:warped_forest",
        173 => "minecraft:basalt_deltas",
        174 => "minecraft:dripstone_caves",
        175 => "minecraft:lush_caves",
        177 => "minecraft:meadow",
        178 => "minecraft:grove",
        179 => "minecraft:snowy_slopes",
        180 => "minecraft:snowcapped_peaks",
        181 => "minecraft:lofty_peaks",
        182 => "minecraft:stony_peaks",
        _ => "minecraft:plains",
    }
}
