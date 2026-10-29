use std::sync::{LazyLock, Mutex};

use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use super::placement_modifier::{self, PlacementModifier};
use crate::util::codec::DecodeResult;
use crate::world::item::component::codec::{Codec, Stream, inline_holder_set};
use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::feature::feature::{self, Feature, FeatureHolder};

pub struct PlacedFeature {
    pub feature: FeatureHolder,
    pub placement: Vec<PlacementModifier>,
}

static PLACED_FEATURES: LazyLock<Vec<PlacedFeature>> = LazyLock::new(|| {
    crate::core::data_registries::json_entries("minecraft:worldgen/placed_feature")
        .iter()
        .map(|(name, json)| {
            let json: Value = serde_json::from_str(json).expect("placed feature json");
            PlacedFeature::parse(&json).unwrap_or_else(|e| panic!("placed feature {name}: {e}"))
        })
        .collect()
});

pub fn placed_feature_index(name: &str) -> Option<u32> {
    let name = resource(name);
    crate::core::data_registries::json_entries("minecraft:worldgen/placed_feature").iter().position(|(n, _)| *n == name).map(|i| i as u32)
}

static INLINE_PLACED_FEATURES: Mutex<Vec<&'static PlacedFeature>> = Mutex::new(Vec::new());

pub fn inline_placed(feature: Feature) -> u32 {
    let placed: &'static PlacedFeature = Box::leak(Box::new(PlacedFeature { feature: FeatureHolder::Direct(Box::new(feature)), placement: Vec::new() }));
    let mut inline = INLINE_PLACED_FEATURES.lock().unwrap();
    inline.push(placed);
    (PLACED_FEATURES.len() + inline.len() - 1) as u32
}

pub fn get(index: u32) -> &'static PlacedFeature {
    match PLACED_FEATURES.get(index as usize) {
        Some(feature) => feature,
        None => INLINE_PLACED_FEATURES.lock().unwrap()[index as usize - PLACED_FEATURES.len()],
    }
}

impl PlacedFeature {
    pub fn get_features(&self, out: &mut Vec<u32>) {
        self.feature.get_features(out);
    }

    pub fn parse(json: &Value) -> Result<PlacedFeature, String> {
        Ok(PlacedFeature {
            feature: FeatureHolder::parse(json.get("feature").ok_or("missing feature")?)?,
            placement: json.get("placement").and_then(Value::as_array).ok_or("missing placement")?.iter().map(PlacementModifier::parse).collect::<Result<_, _>>()?,
        })
    }
}

pub enum PlacedFeatureHolder {
    Reference(u32),
    Direct(Box<PlacedFeature>),
}

pub fn parse_placed_feature_set(json: &Value) -> Result<Vec<PlacedFeatureHolder>, String> {
    let features = match json {
        Value::Array(list) => list.iter().map(PlacedFeatureHolder::parse).collect::<Result<Vec<_>, _>>()?,
        other => vec![PlacedFeatureHolder::parse(other)?],
    };
    if features.is_empty() {
        return Err("features must not be empty".into());
    }
    Ok(features)
}

impl PlacedFeatureHolder {
    pub fn parse(json: &Value) -> Result<PlacedFeatureHolder, String> {
        match json {
            Value::String(name) => placed_feature_index(name).map(PlacedFeatureHolder::Reference).ok_or_else(|| crate::core::data_registries::unknown_reference("minecraft:worldgen/placed_feature", &resource(name))),
            other => Ok(PlacedFeatureHolder::Direct(Box::new(PlacedFeature::parse(other)?))),
        }
    }

    pub fn value(&self) -> &PlacedFeature {
        match self {
            PlacedFeatureHolder::Reference(index) => get(*index),
            PlacedFeatureHolder::Direct(feature) => feature,
        }
    }
}

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(direct_codec);
    &CODEC
}

fn holder_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(holder_codec);
    &CODEC
}

pub fn direct_codec() -> Pair {
    record(vec![req("feature", feature::holder_codec()), req("placement", types::list(placement_modifier::codec()))])
}

pub fn holder_codec() -> Pair {
    (Codec::Holder("minecraft:worldgen/placed_feature", Some(Box::new(Codec::Lazy(direct_codec_lazy)))), Stream::Nbt)
}

fn decode_list(tag: &Nbt) -> DecodeResult<Nbt> {
    inline_holder_set("minecraft:worldgen/placed_feature", tag, &holder_codec_lazy().0, false)
}

fn decode_always_list(tag: &Nbt) -> DecodeResult<Nbt> {
    inline_holder_set("minecraft:worldgen/placed_feature", tag, &holder_codec_lazy().0, true)
}

pub fn list_codec() -> Pair {
    (Codec::Func(decode_list), Stream::Nbt)
}

pub fn list_of_lists_codec() -> Pair {
    types::list((Codec::Func(decode_always_list), Stream::Nbt))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::world::item::component::json_to_nbt;

    fn decode(json: serde_json::Value) -> Result<Nbt, String> {
        crate::nbt::with_json_source(|| direct_codec().0.decode(&json_to_nbt(&json)))
    }

    fn placement(modifier: serde_json::Value) -> Result<Nbt, String> {
        decode(json!({"feature": "minecraft:oak", "placement": [modifier]}))
    }

    #[test]
    fn valid_modifiers_decode() {
        assert!(placement(json!({"type": "minecraft:count", "count": {"type": "minecraft:uniform", "min_inclusive": 1, "max_inclusive": 40}})).is_ok());
        assert!(placement(json!({"type": "minecraft:cuboid", "xz_size": 4, "y_size": 3})).is_ok());
        assert!(placement(json!({"type": "minecraft:fixed_placement", "positions": [[1, 2, 3]]})).is_ok());
        assert!(placement(json!({"type": "minecraft:randomly_selected", "placements": [{"type": "minecraft:in_square"}, {"type": "minecraft:biome"}]})).is_ok());
        assert!(placement(json!({"type": "minecraft:environment_scan", "direction_of_search": "up", "target_condition": {"type": "minecraft:solid"}, "max_steps": 32})).is_ok());
    }

    #[test]
    fn several_modifier_errors_are_joined_in_order() {
        let result = decode(json!({"feature": "minecraft:oak", "placement": [
            {"type": "minecraft:count", "count": -1},
            5,
            {"type": "minecraft:rarity_filter", "chance": 0},
            null,
            {"type": "nope"},
        ]}));
        assert_eq!(
            result.unwrap_err(),
            "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/placement_modifier_type]: minecraft:nope; Not a JSON object: null; Value must be positive: 0; Not a JSON object: 5; Value provider too low: 0 [-1--1]"
        );
    }

    #[test]
    fn environment_scan_reports_fields_in_reverse_order() {
        let result = placement(json!({"type": "minecraft:environment_scan", "direction_of_search": "north", "target_condition": 5, "max_steps": 0}));
        assert_eq!(result.unwrap_err(), "Value 0 outside of range [1:32]; Not a JSON object: 5; Expected a vertical direction");
    }

    #[test]
    fn count_checks_the_provider_bounds_after_its_own_validation() {
        let count = |count: serde_json::Value| placement(json!({"type": "minecraft:count", "count": count})).unwrap_err();
        assert_eq!(count(json!(4097)), "Value provider too high: 4096 [4097-4097]");
        assert_eq!(
            count(json!({"type": "minecraft:weighted_list", "distribution": []})),
            "Failed to parse either. First: Not a number: {\"type\":\"minecraft:weighted_list\",\"distribution\":[]}; Second: Weighted list must contain at least one entry with non-zero weight"
        );
        assert_eq!(
            count(json!({"type": "minecraft:uniform", "min_inclusive": 5, "max_inclusive": 1})),
            "Failed to parse either. First: Not a number: {\"type\":\"minecraft:uniform\",\"min_inclusive\":5,\"max_inclusive\":1}; Second: Max must be at least min, min_inclusive: 5, max_inclusive: 1"
        );
    }

    #[test]
    fn an_invalid_identifier_falls_back_to_the_inline_definition() {
        assert_eq!(decode(json!({"feature": "Minecraft:Oak", "placement": []})).unwrap_err(), "Not a JSON object: \"Minecraft:Oak\"");
        assert_eq!(decode(json!({"feature": "minecraft:oak", "placement": 5})).unwrap_err(), "Not a json array: 5");
    }

    #[test]
    fn nested_inline_feature_errors_come_before_the_placement_errors() {
        let result = decode(json!({"feature": {"type": "minecraft:tree", "trunk_provider": 5}, "placement": [{"type": "minecraft:count", "count": -5}]}));
        let tree = "{\"type\":\"minecraft:tree\",\"trunk_provider\":5}";
        let missing = |key: &str| format!("No key {key} in MapLike[{tree}]");
        let expected = [
            "Value provider too low: 0 [-5--5]".to_string(),
            missing("below_trunk_provider"),
            missing("decorators"),
            missing("minimum_size"),
            missing("foliage_placer"),
            missing("foliage_provider"),
            missing("trunk_placer"),
            "Not a JSON object: 5".to_string(),
            "Not a JSON object: 5".to_string(),
        ];
        assert_eq!(result.unwrap_err(), expected.join("; "));
    }

    #[test]
    fn fixed_placement_positions_must_be_numeric_triples() {
        let positions = |positions: serde_json::Value| placement(json!({"type": "minecraft:fixed_placement", "positions": positions})).unwrap_err();
        assert_eq!(positions(json!([[1, 2, "x"]])), "Some elements are not ints: [1,2,\"x\"]");
        assert_eq!(positions(json!([[1, 2]])), "Input is not a list of 3 ints");
        assert_eq!(positions(json!(5)), "Not a json array: 5");
    }

    #[test]
    fn list_codec_accepts_a_single_inline_definition_or_a_list() {
        let list = |json: serde_json::Value| crate::nbt::with_json_source(|| list_codec().0.decode(&json_to_nbt(&json)));
        assert!(list(json!({"feature": "minecraft:oak", "placement": []})).is_ok());
        assert!(list(json!([{"feature": "minecraft:oak", "placement": []}, {"feature": "minecraft:oak", "placement": []}])).is_ok());
        assert_eq!(list(json!([5])).unwrap_err(), "Not a JSON object: 5");
    }

    #[test]
    fn list_of_lists_codec_never_accepts_a_bare_reference_as_an_element() {
        let lists = |json: serde_json::Value| crate::nbt::with_json_source(|| list_of_lists_codec().0.decode_partial(&json_to_nbt(&json)));
        assert!(lists(json!([[{"feature": "minecraft:oak", "placement": []}], []])).is_success());
        assert_eq!(lists(json!(["minecraft:flower_plain"])).error.unwrap(), "Failed to parse either. First: Not a tag id; Second: Not a json array: \"minecraft:flower_plain\"");
        assert!(lists(json!(["minecraft:flower_plain"])).value.is_some());
    }
}
