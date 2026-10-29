use std::collections::BTreeSet;

use wafer_core::ChunkPos;

use super::structure::{self, StructureHolder};
use super::structure_set;
use super::structures::StructureType;
use crate::util::random::SUM_OF_WEIGHTS_EXCEPTION;
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::types::Pair;
use crate::world::level::chunk::{ChunkGenerator, ChunkStructures};
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::tests::{oracle_dimension_type, oracle_generator};

#[test]
#[ignore]
fn structure_starts_match_vanilla_dump() {
    let dump = std::fs::read_to_string(std::env::var("WAFER_STARTS_DUMP").expect("WAFER_STARTS_DUMP")).unwrap();
    let range: Vec<i32> = std::env::var("WAFER_STARTS_RANGE").expect("WAFER_STARTS_RANGE").split(',').map(|s| s.parse().unwrap()).collect();
    let seed = 12345;
    let implemented = |id: &str| StructureHolder::by_name(id).is_some_and(|holder| !matches!(holder.value().structure_type, StructureType::NotImplemented));
    let expected: BTreeSet<String> = dump
        .lines()
        .filter_map(|line| line.strip_prefix("START "))
        .filter(|line| line.split(' ').nth(2).is_some_and(implemented))
        .map(str::to_owned)
        .collect();
    let generator = ChunkGenerator::Noise(std::sync::Arc::new(oracle_generator(seed)));
    let state = ChunkGeneratorStructureState::create_for_normal(&generator, seed);
    let mut actual = BTreeSet::new();
    for x in range[0]..=range[2] {
        for z in range[1]..=range[3] {
            let pos = ChunkPos::new(x, z);
            let mut structures = ChunkStructures::default();
            generator.create_structures(&state, pos, oracle_dimension_type(), &mut structures, false);
            for (holder, start) in &structures.starts {
                actual.insert(format!("{x} {z} {} {}", holder.key(), crate::nbt::to_string(&start.create_tag(pos))));
            }
        }
    }
    if let Ok(out) = std::env::var("WAFER_STARTS_OUT") {
        std::fs::write(out, actual.iter().map(|line| format!("{line}\n")).collect::<String>()).unwrap();
    }
    let missing: Vec<_> = expected.difference(&actual).collect();
    let extra: Vec<_> = actual.difference(&expected).collect();
    eprintln!("{} expected, {} actual, {} missing, {} extra", expected.len(), actual.len(), missing.len(), extra.len());
    for line in missing.iter().take(5) {
        eprintln!("MISSING {}", &line[..line.len().min(600)]);
    }
    for line in extra.iter().take(5) {
        eprintln!("EXTRA {}", &line[..line.len().min(600)]);
    }
    assert!(missing.is_empty() && extra.is_empty());
}

fn decode(codec: Pair, json: serde_json::Value) -> Result<(), String> {
    crate::core::data_registries::json_entries("minecraft:worldgen/structure");
    crate::nbt::with_json_source(|| codec.0.decode(&json_to_nbt(&json)).map(drop))
}

fn buried_treasure(extra: serde_json::Value) -> serde_json::Value {
    let mut structure = serde_json::json!({"type": "minecraft:buried_treasure", "biomes": "#minecraft:is_ocean", "step": "surface_structures", "spawn_overrides": {}});
    structure.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    structure
}

fn jigsaw(extra: serde_json::Value) -> serde_json::Value {
    let mut structure = buried_treasure(serde_json::json!({
        "type": "minecraft:jigsaw",
        "start_pool": "minecraft:pillager_outpost/base_plates",
        "size": 7,
        "start_height": {"absolute": 0},
        "use_expansion_hack": true,
        "max_distance_from_center": 80,
    }));
    structure.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    structure
}

fn spawns(spawns: serde_json::Value) -> serde_json::Value {
    buried_treasure(serde_json::json!({"spawn_overrides": {"monster": {"bounding_box": "full", "spawns": spawns}}}))
}

fn structure_set(placement: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"structures": [{"structure": "minecraft:shipwreck", "weight": 1}], "placement": placement})
}

fn random_spread(extra: serde_json::Value) -> serde_json::Value {
    let mut placement = serde_json::json!({"type": "minecraft:random_spread", "salt": 5, "spacing": 10, "separation": 2});
    placement.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    structure_set(placement)
}

#[test]
fn structure_settings_report_vanillas_errors() {
    let missing = r#"No key step in MapLike[{"type":"minecraft:buried_treasure"}]; No key spawn_overrides in MapLike[{"type":"minecraft:buried_treasure"}]; No key biomes in MapLike[{"type":"minecraft:buried_treasure"}]"#;
    assert_eq!(decode(structure::direct_codec(), serde_json::json!({"type": "minecraft:buried_treasure"})), Err(missing.into()));
    assert_eq!(
        decode(structure::direct_codec(), serde_json::json!({"type": "minecraft:nope"})),
        Err("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/structure_type]: minecraft:nope".into())
    );
    assert_eq!(decode(structure::direct_codec(), buried_treasure(serde_json::json!({"step": "x"}))), Err("Unknown element name:x".into()));
    let biomes = concat!(
        "Failed to parse either. First: Not a valid resource location: Minecraft:x Non [a-z0-9_.-] character in namespace of identifier: Minecraft:x; ",
        r##"Second: Failed to parse either. First: Not a json array: "#Minecraft:x"; "##,
        "Second: Not a valid resource location: #Minecraft:x Non [a-z0-9_.-] character in namespace of identifier: #Minecraft:x",
    );
    assert_eq!(decode(structure::direct_codec(), buried_treasure(serde_json::json!({"biomes": "#Minecraft:x"}))), Err(biomes.into()));
}

#[test]
fn structure_spawn_overrides_report_vanillas_errors() {
    let pig = |weight: i64| serde_json::json!({"type": "minecraft:pig", "count": 1, "weight": weight});
    assert_eq!(decode(structure::direct_codec(), spawns(serde_json::json!([pig(1)]))), Ok(()));
    let negative = r#"Value must be non-negative: -1 missed input: {"monster":{"bounding_box":"full","spawns":[{"type":"minecraft:pig","count":1,"weight":-1}]}}"#;
    assert_eq!(decode(structure::direct_codec(), spawns(serde_json::json!([pig(-1)]))), Err(negative.into()));
    let overflow = decode(structure::direct_codec(), spawns(serde_json::json!([pig(2147483647), pig(2147483647)])));
    assert!(overflow.unwrap_err().contains(SUM_OF_WEIGHTS_EXCEPTION));
    let count = concat!(
        r#"Failed to parse either. First: Not a number: {"type":"minecraft:uniform","min_inclusive":3,"max_inclusive":1}; "#,
        "Second: Max must be at least min, min_inclusive: 3, max_inclusive: 1 ",
        r#"missed input: {"monster":{"bounding_box":"full","spawns":[{"type":"minecraft:pig","count":{"type":"minecraft:uniform","min_inclusive":3,"max_inclusive":1},"weight":1}]}}"#,
    );
    let uniform = serde_json::json!([{"type": "minecraft:pig", "count": {"type": "minecraft:uniform", "min_inclusive": 3, "max_inclusive": 1}, "weight": 1}]);
    assert_eq!(decode(structure::direct_codec(), spawns(uniform)), Err(count.into()));
    let trapezoid = concat!(
        r#"Failed to parse either. First: Not a number: {"type":"minecraft:trapezoid","min":1,"max":5,"plateau":9}; "#,
        "Second: Plateau can at most be the full span: [1, 5] ",
        r#"missed input: {"monster":{"bounding_box":"full","spawns":[{"type":"minecraft:pig","count":{"type":"minecraft:trapezoid","min":1,"max":5,"plateau":9},"weight":1}]}}"#,
    );
    let plateau = serde_json::json!([{"type": "minecraft:pig", "count": {"type": "minecraft:trapezoid", "min": 1, "max": 5, "plateau": 9}, "weight": 1}]);
    assert_eq!(decode(structure::direct_codec(), spawns(plateau)), Err(trapezoid.into()));
    let empty = concat!(
        r#"Failed to parse either. First: Not a number: {"type":"minecraft:weighted_list","distribution":[]}; "#,
        "Second: Weighted list must contain at least one entry with non-zero weight ",
        r#"missed input: {"monster":{"bounding_box":"full","spawns":[{"type":"minecraft:pig","count":{"type":"minecraft:weighted_list","distribution":[]},"weight":1}]}}"#,
    );
    let distribution = serde_json::json!([{"type": "minecraft:pig", "count": {"type": "minecraft:weighted_list", "distribution": []}, "weight": 1}]);
    assert_eq!(decode(structure::direct_codec(), spawns(distribution)), Err(empty.into()));
    let category = r#"Unknown element name:foo missed input: {"foo":{"bounding_box":"full","spawns":[]}}"#;
    let foo = serde_json::json!({"spawn_overrides": {"foo": {"bounding_box": "full", "spawns": []}}});
    assert_eq!(decode(structure::direct_codec(), buried_treasure(foo)), Err(category.into()));
}

#[test]
fn jigsaw_structures_report_vanillas_errors() {
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({}))), Ok(()));
    let range = "Horizontal structure size including terrain adaptation must not exceed 128";
    let too_wide = serde_json::json!({"max_distance_from_center": 120, "terrain_adaptation": "bury"});
    assert_eq!(decode(structure::direct_codec(), jigsaw(too_wide)), Err(range.into()));
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({"max_distance_from_center": 128}))), Ok(()));
    let partial_biomes = serde_json::json!({"biomes": [5], "max_distance_from_center": 120, "terrain_adaptation": "bury"});
    assert_eq!(decode(structure::direct_codec(), jigsaw(partial_biomes)), Err(format!("Not a string: 5; {range}")));
    let distance = r#"Failed to parse either. First: Value must be within range [1;4064]: 0; Value 129 outside of range [1:128]; Second: Not a number: {"horizontal":129,"vertical":0}"#;
    let far = serde_json::json!({"max_distance_from_center": {"horizontal": 129, "vertical": 0}});
    assert_eq!(decode(structure::direct_codec(), jigsaw(far)), Err(distance.into()));
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({"size": 21}))), Err("Value 21 outside of range [0:20]".into()));
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({"dimension_padding": {"bottom": -1, "top": 2}}))), Ok(()));
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({"dimension_padding": 3, "liquid_settings": "ignore_waterlogging"}))), Ok(()));
    let direct = serde_json::json!({"type": "minecraft:direct"});
    let alias = r#"No key target in MapLike[{"type":"minecraft:direct"}]; No key alias in MapLike[{"type":"minecraft:direct"}]"#;
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({"pool_aliases": [direct]}))), Err(alias.into()));
    let group = serde_json::json!({"type": "minecraft:random_group", "groups": [{"data": [{"type": "minecraft:direct", "alias": "ta:a", "target": "ta:b"}], "weight": 0}]});
    let empty = "Weighted list must contain at least one entry with non-zero weight";
    assert_eq!(decode(structure::direct_codec(), jigsaw(serde_json::json!({"pool_aliases": [group]}))), Err(empty.into()));
}

#[test]
fn other_structures_report_vanillas_errors() {
    let setups = serde_json::json!({"type": "minecraft:ruined_portal", "setups": []});
    assert_eq!(decode(structure::direct_codec(), buried_treasure(setups)), Err("List must have contents".into()));
    let ocean_ruin = serde_json::json!({"type": "minecraft:ocean_ruin", "biome_temp": "warm", "large_probability": 1.0001, "cluster_probability": 0});
    assert_eq!(decode(structure::direct_codec(), buried_treasure(ocean_ruin)), Err("Value 1.0001 outside of range [0.0:1.0]".into()));
    let mineshaft = serde_json::json!({"type": "minecraft:mineshaft", "mineshaft_type": "Normal"});
    assert_eq!(decode(structure::direct_codec(), buried_treasure(mineshaft)), Err("Unknown element name:Normal".into()));
}

#[test]
fn structure_sets_report_vanillas_errors() {
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({}))), Ok(()));
    let equal = serde_json::json!({"spacing": 5, "separation": 5});
    assert_eq!(decode(structure_set::direct_codec(), random_spread(equal)), Err("Spacing has to be larger than separation".into()));
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({"spacing": 4097}))), Err("Value 4097 outside of range [0:4096]".into()));
    let offset = "Position out of range, expected at most 16: Vec3i{x=17, y=0, z=0}";
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({"locate_offset": [17, 0, 0]}))), Err(offset.into()));
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({"locate_offset": [-16, 16, 0]}))), Ok(()));
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({"locate_offset": "x"}))), Err(r#"Not a json array: "x""#.into()));
    let not_ints = r#"Some elements are not ints: ["a",1,1]"#;
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({"locate_offset": ["a", 1, 1]}))), Err(not_ints.into()));
    assert_eq!(decode(structure_set::direct_codec(), random_spread(serde_json::json!({"locate_offset": [1, 2]}))), Err("Input is not a list of 3 ints".into()));
    let zone = |other_set: serde_json::Value, chunk_count: i64| random_spread(serde_json::json!({"exclusion_zone": {"other_set": other_set, "chunk_count": chunk_count}}));
    assert_eq!(decode(structure_set::direct_codec(), zone(serde_json::json!("minecraft:villages"), 16)), Ok(()));
    assert_eq!(decode(structure_set::direct_codec(), zone(serde_json::json!(5), 16)), Err("Inline definitions not allowed here".into()));
    assert_eq!(decode(structure_set::direct_codec(), zone(serde_json::json!({}), 16)), Err("Inline definitions not allowed here".into()));
    assert_eq!(decode(structure_set::direct_codec(), zone(serde_json::json!("minecraft:villages"), 17)), Err("Value 17 outside of range [1:16]".into()));
    let placement = serde_json::json!({"type": "minecraft:concentric_rings", "distance": 1024, "spread": 1024, "count": 4096, "preferred_biomes": [], "salt": 0});
    let rings = "Value 4096 outside of range [1:4095]; Value 1024 outside of range [0:1023]; Value 1024 outside of range [0:1023]";
    assert_eq!(decode(structure_set::direct_codec(), structure_set(placement)), Err(rings.into()));
    let kind = serde_json::json!({"type": "minecraft:nope"});
    let unknown = "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/structure_placement]: minecraft:nope";
    assert_eq!(decode(structure_set::direct_codec(), structure_set(kind)), Err(unknown.into()));
    let origin = serde_json::json!({"type": "minecraft:dimension_origin", "salt": "ignored"});
    assert_eq!(decode(structure_set::direct_codec(), structure_set(origin)), Ok(()));
    let weight = serde_json::json!({"structures": [{"structure": "minecraft:shipwreck", "weight": 0}], "placement": {"type": "minecraft:dimension_origin"}});
    assert_eq!(decode(structure_set::direct_codec(), weight), Err("Value must be positive: 0".into()));
}

#[test]
fn negative_zero_is_outside_of_a_float_range_starting_at_zero() {
    let placement = r#"{"type":"minecraft:random_spread","salt":5,"spacing":10,"separation":2,"frequency":-0.0}"#;
    let json = structure_set(serde_json::from_str(placement).unwrap());
    assert_eq!(decode(structure_set::direct_codec(), json), Err("Value -0.0 outside of range [0.0:1.0]".into()));
}
