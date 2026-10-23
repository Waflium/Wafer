use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::CHUNK;

struct Conversion {
    biome_mapping: &'static [(&'static [&'static str], &'static str)],
    fallback: &'static str,
}

impl Conversion {
    const fn trivial(result: &'static str) -> Conversion {
        Conversion { biome_mapping: &[], fallback: result }
    }

    const fn biome_mapped(mapping: &'static [(&'static [&'static str], &'static str)], fallback: &'static str) -> Conversion {
        Conversion { biome_mapping: mapping, fallback }
    }

    fn mapping_for(&self, biome: &str) -> Option<&'static str> {
        self.biome_mapping.iter().find(|(biomes, _)| biomes.contains(&biome)).map(|(_, mapping)| *mapping)
    }
}

static CONVERSION_MAP: [(&str, Conversion); 18] = [
    ("mineshaft", Conversion::biome_mapped(&[(&["minecraft:badlands", "minecraft:eroded_badlands", "minecraft:wooded_badlands"], "minecraft:mineshaft_mesa")], "minecraft:mineshaft")),
    ("shipwreck", Conversion::biome_mapped(&[(&["minecraft:beach", "minecraft:snowy_beach"], "minecraft:shipwreck_beached")], "minecraft:shipwreck")),
    (
        "ocean_ruin",
        Conversion::biome_mapped(&[(&["minecraft:warm_ocean", "minecraft:lukewarm_ocean", "minecraft:deep_lukewarm_ocean"], "minecraft:ocean_ruin_warm")], "minecraft:ocean_ruin_cold"),
    ),
    (
        "village",
        Conversion::biome_mapped(
            &[
                (&["minecraft:desert"], "minecraft:village_desert"),
                (&["minecraft:savanna"], "minecraft:village_savanna"),
                (&["minecraft:snowy_plains"], "minecraft:village_snowy"),
                (&["minecraft:taiga"], "minecraft:village_taiga"),
            ],
            "minecraft:village_plains",
        ),
    ),
    (
        "ruined_portal",
        Conversion::biome_mapped(
            &[
                (&["minecraft:desert"], "minecraft:ruined_portal_desert"),
                (
                    &[
                        "minecraft:badlands",
                        "minecraft:eroded_badlands",
                        "minecraft:wooded_badlands",
                        "minecraft:windswept_hills",
                        "minecraft:windswept_forest",
                        "minecraft:windswept_gravelly_hills",
                        "minecraft:savanna_plateau",
                        "minecraft:windswept_savanna",
                        "minecraft:stony_shore",
                        "minecraft:meadow",
                        "minecraft:frozen_peaks",
                        "minecraft:jagged_peaks",
                        "minecraft:stony_peaks",
                        "minecraft:snowy_slopes",
                    ],
                    "minecraft:ruined_portal_mountain",
                ),
                (&["minecraft:bamboo_jungle", "minecraft:jungle", "minecraft:sparse_jungle"], "minecraft:ruined_portal_jungle"),
                (
                    &[
                        "minecraft:deep_frozen_ocean",
                        "minecraft:deep_cold_ocean",
                        "minecraft:deep_ocean",
                        "minecraft:deep_lukewarm_ocean",
                        "minecraft:frozen_ocean",
                        "minecraft:ocean",
                        "minecraft:cold_ocean",
                        "minecraft:lukewarm_ocean",
                        "minecraft:warm_ocean",
                    ],
                    "minecraft:ruined_portal_ocean",
                ),
            ],
            "minecraft:ruined_portal",
        ),
    ),
    ("pillager_outpost", Conversion::trivial("minecraft:pillager_outpost")),
    ("mansion", Conversion::trivial("minecraft:mansion")),
    ("jungle_pyramid", Conversion::trivial("minecraft:jungle_pyramid")),
    ("desert_pyramid", Conversion::trivial("minecraft:desert_pyramid")),
    ("igloo", Conversion::trivial("minecraft:igloo")),
    ("swamp_hut", Conversion::trivial("minecraft:swamp_hut")),
    ("stronghold", Conversion::trivial("minecraft:stronghold")),
    ("monument", Conversion::trivial("minecraft:monument")),
    ("fortress", Conversion::trivial("minecraft:fortress")),
    ("endcity", Conversion::trivial("minecraft:end_city")),
    ("buried_treasure", Conversion::trivial("minecraft:buried_treasure")),
    ("nether_fossil", Conversion::trivial("minecraft:nether_fossil")),
    ("bastion_remnant", Conversion::trivial("minecraft:bastion_remnant")),
];

pub struct StructuresBecomeConfiguredFix {
    base: DataFixBase,
}

impl StructuresBecomeConfiguredFix {
    pub fn new(output_schema: &Arc<Schema>) -> StructuresBecomeConfiguredFix {
        StructuresBecomeConfiguredFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for StructuresBecomeConfiguredFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let chunk_type = self.get_input_schema().get_type(CHUNK);
        let new_chunk_type = self.get_input_schema().get_type(CHUNK);
        self.write_fix_and_read("StucturesToConfiguredStructures", chunk_type, new_chunk_type, fix)
    }
}

fn fix(chunk: Dynamic) -> Dynamic {
    let creator = chunk.clone();
    chunk.update("structures", |structures| structures.update("starts", |starts| update_starts(starts, &creator)).update("References", |references| update_references(references, &creator)))
}

fn update_starts(starts: Dynamic, chunk: &Dynamic) -> Dynamic {
    let values = starts.get_map_values().into_result().unwrap_or_default();
    let mut new_map: Vec<(Dynamic, Dynamic)> = Vec::new();
    for (key, start) in values {
        if start.get("id").as_string_or("INVALID") != "INVALID" {
            match find_updated_structure_type(&key, chunk) {
                None => warn!("Encountered unknown structure in datafixer: {}", key.as_string_or("<missing key>")),
                Some(new_key) => {
                    if !new_map.iter().any(|(existing, _)| *existing == new_key) {
                        let new_start = start.set("id", new_key.clone());
                        new_map.push((new_key, new_start));
                    }
                }
            }
        }
    }
    chunk.create_map(new_map)
}

fn update_references(references: Dynamic, chunk: &Dynamic) -> Dynamic {
    let values = references.get_map_values().into_result().unwrap_or_default();
    let mut new_map: Vec<(Dynamic, Dynamic)> = Vec::new();
    for (key, ref_list) in values {
        if !ref_list.as_long_stream().is_empty() {
            match find_updated_structure_type(&key, chunk) {
                None => warn!("Encountered unknown structure in datafixer: {}", key.as_string_or("<missing key>")),
                Some(new_key) => match new_map.iter_mut().find(|(existing, _)| *existing == new_key) {
                    None => new_map.push((new_key, ref_list)),
                    Some((_, old_ref_list)) => {
                        let mut combined = old_ref_list.as_long_stream();
                        combined.extend(ref_list.as_long_stream());
                        *old_ref_list = ref_list.create_long_list(combined);
                    }
                },
            }
        }
    }
    chunk.create_map(new_map)
}

fn find_updated_structure_type(dynamic_key: &Dynamic, chunk: &Dynamic) -> Option<Dynamic> {
    let key = dynamic_key.as_string_or("UNKNOWN").to_lowercase();
    let conversion = &CONVERSION_MAP.iter().find(|(name, _)| *name == key)?.1;
    let mut resulting_id = conversion.fallback;
    if !conversion.biome_mapping.is_empty() && let Some(result) = guess_configuration(chunk, conversion) {
        resulting_id = result;
    }
    Some(chunk.create_string(resulting_id))
}

fn guess_configuration(chunk: &Dynamic, conversion: &Conversion) -> Option<&'static str> {
    let mut matches: Vec<(&'static str, i32)> = Vec::new();
    for section in chunk.get("sections").as_list(|section| section) {
        for biome in section.get("biomes").get("palette").as_list(|biome| biome) {
            if let Some(mapping) = conversion.mapping_for(&biome.as_string_or("")) {
                match matches.iter_mut().find(|(key, _)| *key == mapping) {
                    Some((_, count)) => *count += 1,
                    None => matches.push((mapping, 1)),
                }
            }
        }
    }
    let mut best: Option<(&'static str, i32)> = None;
    for entry in matches {
        if best.is_none_or(|(_, count)| entry.1 > count) {
            best = Some(entry);
        }
    }
    best.map(|(key, _)| key)
}
