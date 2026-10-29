use serde_json::Value;

use super::carver;
use super::density_function::{self, density_functions};
use super::heightproviders;
use super::material::condition::material_condition;
use super::material::rule::material_rule;
use super::material::vertical_anchor_codec;
use super::synth::parameters_codec;
use super::{noise_router_codec, noise_settings_codec, spawn_target_point_codec};
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::types::Pair;

const CAVE: &str = r#"{"type":"minecraft:cave","count":{"type":"minecraft:very_biased_to_bottom","max_inclusive":14,"min_inclusive":0},"floor_level":{"type":"minecraft:uniform","max_exclusive":-0.4,"min_inclusive":-1.0},"horizontal_radius_multiplier":{"type":"minecraft:uniform","max_exclusive":1.4,"min_inclusive":0.7},"probability":0.15,"room_vertical_radius_multiplier":{"type":"minecraft:uniform","max_exclusive":0.9,"min_inclusive":0.1},"thickness":{"type":"minecraft:trapezoid","max":3.0,"min":0.0,"plateau":1.0},"vertical_radius_multiplier":{"type":"minecraft:uniform","max_exclusive":1.3,"min_inclusive":0.8},"weird_thickness_bias":true,"y":{"type":"minecraft:uniform","max_inclusive":{"absolute":180},"min_inclusive":{"above_bottom":8}}}"#;

fn decode(codec: Pair, json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(json).unwrap();
    crate::core::data_registries::json_entries("minecraft:worldgen/density_function");
    crate::nbt::with_json_source(|| codec.0.decode(&json_to_nbt(&value))).err()
}

fn reports(codec: Pair, json: &str, expected: &str) {
    assert_eq!(decode(codec, json).as_deref(), Some(expected), "{json}");
}

fn accepts(codec: Pair, json: &str) {
    assert_eq!(decode(codec, json), None, "{json}");
}

fn either_failure(json: &str, second: &str) -> String {
    format!("Failed to parse either. First: Not a number: {json}; Second: {second}")
}

#[test]
fn density_function_validators_report_vanilla_messages() {
    let clamp = r#"{"type":"minecraft:clamp","input":1,"min":2,"max":1}"#;
    reports(density_functions::direct_codec(), clamp, &either_failure(clamp, "min (2.0) must be less than or equal to max (1.0)"));
    let gradient = r#"{"type":"minecraft:gradient","axis":"y","from_coordinate":5,"to_coordinate":5,"from_value":0,"to_value":1}"#;
    reports(density_functions::direct_codec(), gradient, &either_failure(gradient, "from_coordinate cannot be equal to to_coordinate"));
    let unordered = r#"{"type":"minecraft:interval_select","input":1,"thresholds":[1,0],"functions":[1,2,3]}"#;
    reports(density_functions::direct_codec(), unordered, &either_failure(unordered, "Threshold values must be ordered from smallest to largest"));
    let sizes = r#"{"type":"minecraft:interval_select","input":1,"thresholds":[0,1],"functions":[1,2]}"#;
    reports(density_functions::direct_codec(), sizes, &either_failure(sizes, "Expected 1 thresholds for 2 functions, but got 2"));
    let lower_bound = r#"{"type":"minecraft:find_top_surface","density":1,"upper_bound":2,"lower_bound":4063,"cell_height":8}"#;
    reports(density_functions::direct_codec(), lower_bound, &either_failure(lower_bound, "Value 4063 outside of range [-4064:4062]"));
    accepts(density_functions::direct_codec(), r#"{"type":"minecraft:clamp","input":1,"min":-1,"max":1}"#);
}

#[test]
fn density_function_values_keep_partial_results_and_range_messages() {
    reports(
        density_functions::direct_codec(),
        r#"{"type":"minecraft:distance_to_point","point":[1,2,3,4],"metric":"manhattan"}"#,
        "Input is not a list of 3 ints",
    );
    let point = r#"{"type":"minecraft:distance_to_point","point":[0,0,"x"],"metric":"manhattan"}"#;
    reports(density_functions::direct_codec(), point, &either_failure(point, r#"Some elements are not ints: [0,0,"x"]"#));
    reports(density_functions::direct_codec(), "-1000001", "Failed to parse either. First: Value -1000001.0 outside of range [-1000000.0:1000000.0]; Second: Not a JSON object: -1000001");
    accepts(density_functions::direct_codec(), "1000000");
    accepts(density_functions::direct_codec(), r#"{"type":"minecraft:spline","spline":1e300}"#);
    let spline = r#"{"type":"minecraft:spline","spline":"x"}"#;
    reports(
        density_functions::direct_codec(),
        spline,
        &either_failure(spline, r#"Failed to parse either. First: Not a number: "x"; Second: Not a JSON object: "x""#),
    );
}

#[test]
fn density_function_references_fall_back_to_inline_definitions_for_invalid_identifiers() {
    reports(
        density_function::holder_codec(),
        r#""Minecraft:Stone""#,
        r#"Failed to parse either. First: Not a number: "Minecraft:Stone"; Second: Not a JSON object: "Minecraft:Stone""#,
    );
    accepts(density_function::holder_codec(), "5");
}

#[test]
fn noise_parameters_report_vanilla_messages() {
    reports(parameters_codec(), r#"{"base_octave":0,"base_amplitude":0}"#, "Value 0.0 outside of range [9.999999747378752E-6:1000000.0]");
    reports(parameters_codec(), r#"{"base_octave":33}"#, "Value 33 outside of range [-32:32]");
    reports(parameters_codec(), r#"{"base_octave":0,"normalize":"x"}"#, r#"Failed to parse either. First: Not a boolean: "x"; Second: Invalid normalization type: x"#);
    reports(parameters_codec(), r#"{"base_octave":0,"normalize":1}"#, "Failed to parse either. First: Not a boolean: 1; Second: Not a string: 1");
    reports(parameters_codec(), r#"{"base_octave":0,"amplitude_modifiers":[1,2]}"#, "amplitude_modifiers had size 2, but octave_count was 1");
    reports(
        parameters_codec(),
        r#"{"base_octave":0,"octave_count":2,"amplitude_modifiers":[1,"a",-1]}"#,
        r#"Value -1.0 outside of range [0.0:1000000.0]; Not a number: "a"; amplitude_modifiers had size 1, but octave_count was 2"#,
    );
    accepts(parameters_codec(), r#"{"base_octave":-3,"octave_count":2,"amplitude_modifiers":[1,1],"normalize":"legacy"}"#);
}

#[test]
fn vertical_anchors_follow_the_xor_of_their_four_forms() {
    reports(
        vertical_anchor_codec(),
        r#"{"absolute":1,"above_bottom":2}"#,
        r#"Both alternatives read successfully, can not pick the correct one; first: (Left[1 absolute], {"absolute":1,"above_bottom":2}) second: (Right[Left[2 above bottom]], {"absolute":1,"above_bottom":2})"#,
    );
    reports(
        vertical_anchor_codec(),
        "{}",
        "No key relative_to_sea_level in MapLike[{}]; No key below_top in MapLike[{}]; No key above_bottom in MapLike[{}]; No key absolute in MapLike[{}]",
    );
    reports(
        vertical_anchor_codec(),
        r#"{"absolute":-2033}"#,
        r#"No key relative_to_sea_level in MapLike[{"absolute":-2033}]; No key below_top in MapLike[{"absolute":-2033}]; No key above_bottom in MapLike[{"absolute":-2033}]; Value -2033 outside of range [-2032:2031]"#,
    );
    accepts(vertical_anchor_codec(), r#"{"below_top":5}"#);
}

#[test]
fn height_providers_are_a_constant_anchor_or_a_typed_provider() {
    accepts(heightproviders::codec(), r#"{"absolute":5}"#);
    accepts(heightproviders::codec(), r#"{"type":"minecraft:trapezoid","min_inclusive":{"absolute":0},"max_inclusive":{"absolute":10}}"#);
    let empty = r#"{"type":"minecraft:weighted_list","distribution":[]}"#;
    reports(
        heightproviders::codec(),
        empty,
        &format!(
            "Failed to parse either. First: No key relative_to_sea_level in MapLike[{empty}]; No key below_top in MapLike[{empty}]; No key above_bottom in MapLike[{empty}]; No key absolute in MapLike[{empty}]; Second: Weighted list must contain at least one entry with non-zero weight"
        ),
    );
}

#[test]
fn noise_settings_parts_report_vanilla_messages() {
    reports(noise_settings_codec(), r#"{"min_y":-2032,"height":4080}"#, "Value 4080 outside of range [0:4064]");
    reports(noise_settings_codec(), r#"{"min_y":2031,"height":16}"#, "min_y + height cannot be higher than: 2032");
    reports(noise_settings_codec(), r#"{"min_y":0,"height":17}"#, "height has to be a multiple of 16");
    reports(noise_settings_codec(), r#"{"min_y":1,"height":16}"#, "min_y has to be a multiple of 16");
    accepts(noise_settings_codec(), r#"{"min_y":-64,"height":384}"#);
    reports(
        spawn_target_point_codec(),
        r#"{"minecraft:overworld/continents":[-1,1,2,3]}"#,
        r#"Value 3.0 outside of range [-2.0:2.0]; Input is not a list of 2 elements missed input: {"minecraft:overworld/continents":[-1,1,2,3]}"#,
    );
    reports(
        spawn_target_point_codec(),
        r#"{"minecraft:overworld/continents":[1,-1]}"#,
        r#"Failed to parse either. First: Not a number: [1,-1]; Second: Failed to parse either. First: Cannon construct interval, min > max (1.0 > -1.0); Second: Not a JSON object: [1,-1] missed input: {"minecraft:overworld/continents":[1,-1]}"#,
    );
    accepts(spawn_target_point_codec(), r#"{"minecraft:overworld/continents":[-1,1],"minecraft:overworld/erosion":{"min":0,"max":0.5},"minecraft:overworld/depth":0.25}"#);
    reports(
        noise_router_codec(),
        "{}",
        "No key final_density in MapLike[{}]; No key chunk_surface_level in MapLike[{}]; No key ridges in MapLike[{}]; No key depth in MapLike[{}]; No key erosion in MapLike[{}]; No key continents in MapLike[{}]; No key vegetation in MapLike[{}]; No key temperature in MapLike[{}]",
    );
}

#[test]
fn material_rules_and_conditions_dispatch_on_their_type_registries() {
    reports(
        material_rule::direct_codec(),
        r#"{"type":"minecraft:foo"}"#,
        "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/material_rule_type]: minecraft:foo",
    );
    reports(
        material_condition::direct_codec(),
        r#"{"type":"minecraft:y_above","anchor":{"absolute":0},"surface_depth_multiplier":21,"add_stone_depth":false}"#,
        "Value 21 outside of range [-20:20]",
    );
    reports(
        material_condition::direct_codec(),
        r#"{"type":"minecraft:stone_depth","offset":0,"add_surface_depth":true,"secondary_depth_range":1,"surface_type":"wall"}"#,
        "Unknown element name:wall",
    );
    accepts(material_rule::direct_codec(), r#"{"type":"minecraft:sequence","sequence":[{"type":"minecraft:bandlands"},{"type":"minecraft:block","result_state":"minecraft:stone"}]}"#);
    accepts(material_condition::direct_codec(), r#"{"type":"minecraft:not","invert":{"type":"minecraft:steep"}}"#);
}

#[test]
fn carvers_validate_their_value_providers() {
    let cave = |field: &str, value: &str| {
        let mut json: Value = serde_json::from_str(CAVE).unwrap();
        json[field] = serde_json::from_str(value).unwrap();
        json.to_string()
    };
    accepts(carver::direct_codec(), &cave("probability", "0.5"));
    reports(carver::direct_codec(), &cave("count", "-1"), "Value provider too low: 0 [-1--1]");
    reports(carver::direct_codec(), &cave("thickness", "-0.5"), "Value provider too low: 0.0 [-0.5--0.5]");
    reports(carver::direct_codec(), &cave("floor_level", "2"), "Value provider too high: 1.0 [2.0-2.0]");
    let uniform = r#"{"type":"minecraft:uniform","min_inclusive":0.5,"max_exclusive":0.5}"#;
    reports(carver::direct_codec(), &cave("thickness", uniform), &either_failure(uniform, "Max must be larger than min, min: 0.5, max: 0.5"));
    let count = r#"{"type":"minecraft:uniform","min_inclusive":5,"max_inclusive":3}"#;
    reports(carver::direct_codec(), &cave("count", count), &either_failure(count, "Max must be at least min, min_inclusive: 5, max_inclusive: 3"));
}

#[test]
fn block_predicate_offsets_are_limited_to_sixteen_blocks() {
    let predicate = super::blockpredicates::block_predicate::codec;
    reports(predicate(), r#"{"type":"minecraft:matching_blocks","blocks":"minecraft:water","offset":[1000,1,0]}"#, "Position out of range, expected at most 16: Vec3i{x=1000, y=1, z=0}");
    reports(predicate(), r#"{"type":"minecraft:solid","offset":[0,0,0,0]}"#, "Input is not a list of 3 ints");
    accepts(predicate(), r#"{"type":"minecraft:solid","offset":[16,-16,0]}"#);
    accepts(predicate(), r#"{"type":"minecraft:unobstructed","offset":[1000,0,0]}"#);
    reports(
        predicate(),
        r#"{"type":"minecraft:volume_match","min":[1,0,0],"max":[0,0,0],"match":{"type":"minecraft:true"}}"#,
        "min bound cannot be larger than max bound",
    );
    accepts(predicate(), r#"{"type":"minecraft:volume_match","min":[0,0,0],"max":[16,0,0],"match":{"type":"minecraft:true"}}"#);
}

#[test]
fn carver_lists_accept_inline_carvers_and_report_vanilla_messages() {
    reports(
        carver::list_codec(),
        r##""#Minecraft:nope""##,
        r##"Failed to parse either. First: Not a valid resource location: Minecraft:nope Non [a-z0-9_.-] character in namespace of identifier: Minecraft:nope; Second: Failed to parse either. First: Not a json array: "#Minecraft:nope"; Second: Not a JSON object: "#Minecraft:nope""##,
    );
    reports(
        carver::list_codec(),
        "5",
        "Failed to parse either. First: Not a string: 5; Second: Failed to parse either. First: Not a json array: 5; Second: Not a JSON object: 5",
    );
    reports(carver::list_codec(), r#"[{"type":"minecraft:bogus"}]"#, "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/carver_type]: minecraft:bogus");
    accepts(carver::list_codec(), &format!("[{CAVE}]"));
    accepts(carver::list_codec(), CAVE);
}
