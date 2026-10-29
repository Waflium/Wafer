use serde_json::{Value, json};

use super::feature::direct_codec;
use crate::world::item::component::json_to_nbt;

fn decode(json: Value) -> Result<(), String> {
    crate::nbt::with_json_source(|| direct_codec().0.decode(&json_to_nbt(&json)).map(drop))
}

fn error(json: Value) -> String {
    decode(json).expect_err("a decoding error")
}

#[test]
fn unit_features_ignore_their_fields() {
    assert!(decode(json!({"type": "minecraft:no_op", "x": 1})).is_ok());
    assert!(decode(json!({"type": "minecraft:vines"})).is_ok());
}

#[test]
fn unknown_and_missing_types_are_rejected() {
    assert_eq!(error(json!({"type": "minecraft:bogus"})), "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/feature_type]: minecraft:bogus");
    assert_eq!(error(json!({"probability": 1})), "No key type in MapLike[{\"probability\":1}]");
    assert_eq!(error(json!("minecraft:bamboo")), "Not a JSON object: \"minecraft:bamboo\"");
}

#[test]
fn float_ranges_use_total_ordering() {
    assert!(decode(json!({"type": "minecraft:bamboo", "probability": 1.0})).is_ok());
    assert_eq!(error(json!({"type": "minecraft:bamboo", "probability": -0.0})), "Value -0.0 outside of range [0.0:1.0]");
    assert_eq!(error(json!({"type": "minecraft:bamboo", "probability": 1.1})), "Value 1.1 outside of range [0.0:1.0]");
}

#[test]
fn int_ranges_report_every_failing_field_in_reverse_order() {
    let json = json!({"type": "minecraft:sculk_patch", "charge_count": 33, "amount_per_charge": 501, "spread_attempts": 65, "growth_rounds": 9, "spread_rounds": 9});
    assert_eq!(
        error(json),
        "Value 9 outside of range [0:8]; Value 9 outside of range [0:8]; Value 65 outside of range [1:64]; Value 501 outside of range [1:500]; Value 33 outside of range [1:32]"
    );
}

#[test]
fn floats_beyond_the_float_range_are_infinite() {
    let json = json!({"type": "minecraft:large_dripstone", "replaceable_blocks": [], "column_radius": 1, "height_scale": 1e39, "max_column_radius_to_cave_height_ratio": 1,
        "stalactite_bluntness": 1, "stalagmite_bluntness": 1, "wind_speed": 1, "min_radius_for_wind": 0, "min_bluntness_for_wind": 0});
    assert_eq!(error(json), "Value provider too high: 20.0 [Infinity-Infinity]");
}

#[test]
fn end_gateway_exit_must_be_three_ints() {
    let gateway = |exit: Value| json!({"type": "minecraft:end_gateway", "exact": true, "exit": exit});
    assert!(decode(gateway(json!([1, 2, 3]))).is_ok());
    assert_eq!(error(gateway(json!([1, 2]))), "Input is not a list of 3 ints");
    assert_eq!(error(gateway(json!([1, "a", 3]))), "Some elements are not ints: [1,\"a\",3]");
    assert_eq!(error(gateway(json!("x"))), "Not a json array: \"x\"");
}

#[test]
fn state_providers_and_block_states_decode_inline() {
    assert_eq!(error(json!({"type": "minecraft:fill_layer", "height": 4065, "state": {"id": "x"}})), "Failed to parse either. First: Not a string: {\"id\":\"x\"}; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: minecraft:x; Value 4065 outside of range [0:4064]");
    assert_eq!(error(json!({"type": "minecraft:fill_layer", "height": -1, "state": "minecraft:stone"})), "Value -1 outside of range [0:4064]");
}

#[test]
fn multiface_growth_requires_a_spreadeable_block() {
    let growth = |block: &str| json!({"type": "minecraft:multiface_growth", "block": block, "can_be_placed_on": []});
    assert!(decode(growth("minecraft:glow_lichen")).is_ok());
    assert!(decode(growth("minecraft:sculk_vein")).is_ok());
    assert_eq!(error(growth("minecraft:stone")), "Growth block should be a multiface spreadeable block");
    assert_eq!(error(growth("x:y")), "Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: x:y");
}

#[test]
fn fossil_lists_must_be_non_empty_and_equally_long() {
    let fossil = |fossil: Value, overlay: Value| {
        json!({"type": "minecraft:fossil", "fossil_structures": fossil, "overlay_structures": overlay, "fossil_processors": [], "overlay_processors": [], "max_empty_corners_allowed": 4})
    };
    assert_eq!(error(fossil(json!(["a:b"]), json!(["c:d", "e:f"]))), "Fossil structure lists must be equal lengths");
    assert_eq!(error(fossil(json!([]), json!([]))), "List must have contents; List must have contents");
    assert_eq!(error(fossil(json!("a:b"), json!("c:d"))), "Not a json array: \"c:d\"; Not a json array: \"a:b\"");
}

#[test]
fn single_block_pillar_direction_must_be_vertical() {
    let pillar = |direction: &str| json!({"type": "minecraft:single_block_pillar", "block": {"type": "minecraft:simple", "state": {"id": "minecraft:stone"}}, "direction": direction});
    assert!(decode(pillar("up")).is_ok());
    assert_eq!(error(pillar("north")), "Expected a vertical direction");
    assert_eq!(error(pillar("x")), "Unknown element name:x");
}

#[test]
fn spring_state_is_a_fluid_state() {
    let spring = |state: Value| json!({"type": "minecraft:spring_feature", "state": state, "valid_blocks": []});
    assert!(decode(spring(json!({"id": "minecraft:flowing_water", "properties": {"falling": "true", "level": 3}}))).is_ok());
    assert_eq!(error(spring(json!({"id": "minecraft:stone"}))), "Unknown registry key in ResourceKey[minecraft:root / minecraft:fluid]: minecraft:stone");
    assert_eq!(error(spring(json!("minecraft:water"))), "Not a JSON object: \"minecraft:water\"");
}

#[test]
fn template_rotations_are_enum_names() {
    let template = |rotations: Value| json!({"type": "minecraft:template", "templates": [{"data": {"id": "a:b", "rotations": rotations}, "weight": 1}]});
    assert!(decode(template(json!(["none", "clockwise_90", "180", "counterclockwise_90"]))).is_ok());
    assert_eq!(error(template(json!(["CLOCKWISE_90"]))), "Unknown element name:CLOCKWISE_90");
    assert_eq!(error(template(json!("180"))), "Not a json array: \"180\"");
}

#[test]
fn geode_collects_every_nested_error() {
    let provider = json!({"type": "minecraft:simple", "state": {"id": "minecraft:stone"}});
    let json = json!({"type": "minecraft:geode", "invalid_blocks_threshold": 1,
        "blocks": {"filling_provider": provider, "inner_layer_provider": provider, "alternate_inner_layer_provider": provider, "middle_layer_provider": provider,
            "outer_layer_provider": provider, "inner_placements": [], "cannot_replace": [], "invalid_blocks": []},
        "layers": {}, "crack": {}});
    assert_eq!(error(json), "List must have contents");
    assert_eq!(error(json!({"type": "minecraft:geode"})), "No key invalid_blocks_threshold in MapLike[{\"type\":\"minecraft:geode\"}]; No key crack in MapLike[{\"type\":\"minecraft:geode\"}]; No key layers in MapLike[{\"type\":\"minecraft:geode\"}]; No key blocks in MapLike[{\"type\":\"minecraft:geode\"}]");
}

#[test]
fn value_provider_ranges_name_the_effective_bounds() {
    let delta = |size: Value| json!({"type": "minecraft:delta_feature", "contents": "minecraft:stone", "rim": "minecraft:stone", "size": size, "rim_size": 0});
    assert!(decode(delta(json!({"type": "minecraft:uniform", "min_inclusive": 0, "max_inclusive": 16}))).is_ok());
    assert_eq!(error(delta(json!(17))), "Value provider too high: 16 [17-17]");
    assert_eq!(error(delta(json!(-1))), "Value provider too low: 0 [-1--1]");
    assert_eq!(
        error(delta(json!({"type": "minecraft:uniform", "min_inclusive": 5, "max_inclusive": 4}))),
        "Failed to parse either. First: Not a number: {\"type\":\"minecraft:uniform\",\"min_inclusive\":5,\"max_inclusive\":4}; Second: Max must be at least min, min_inclusive: 5, max_inclusive: 4"
    );
}

#[test]
fn holders_fall_back_to_the_inline_codec_for_invalid_identifiers() {
    let pile = |provider: Value| json!({"type": "minecraft:block_pile", "state_provider": provider});
    assert_eq!(error(pile(json!("Minecraft:Stone"))), "Not a JSON object: \"Minecraft:Stone\"; Not a JSON object: \"Minecraft:Stone\"");
    assert_eq!(error(pile(json!("#a:b"))), "Not a JSON object: \"#a:b\"; Not a JSON object: \"#a:b\"");
}

#[test]
fn placed_feature_sets_must_not_be_empty() {
    for ty in ["sequence", "overlay", "simple_random_selector"] {
        assert_eq!(error(json!({"type": format!("minecraft:{ty}"), "features": []})), "List must have contents");
    }
}

#[test]
fn ints_wrap_like_java_numbers() {
    let square = |height: Value| json!({"type": "minecraft:projected_random_patchy_square", "block": {"type": "minecraft:simple", "state": {"id": "minecraft:stone"}},
        "project_through": {"type": "minecraft:true"}, "size": 1, "max_projection_height": height});
    assert!(decode(square(json!(4294967296.5))).is_ok());
    assert_eq!(error(square(json!(2147483648u64))), "Value must be non-negative: -2147483648");
}
