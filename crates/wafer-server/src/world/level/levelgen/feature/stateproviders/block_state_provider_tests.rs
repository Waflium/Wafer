use serde_json::json;
use wafer_core::BlockPos;
use wafer_core::random::LegacyRandomSource;
use wafer_data::blocks;

use super::block_state_provider::BlockStateProvider;
use crate::server::level::testing;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;

fn state_of(json: serde_json::Value) -> Vec<wafer_data::BlockState> {
    let provider = BlockStateProvider::parse(&json).unwrap();
    let mut level = testing::level();
    let mut random = LegacyRandomSource::new(1);
    (0..64).filter_map(|_| provider.get_optional_state(&mut level, &mut random, BlockPos::ZERO)).collect()
}

#[test]
fn random_block_accepts_a_block_list() {
    let states = state_of(json!({"type": "minecraft:random_block", "blocks": ["minecraft:stone", "minecraft:dirt"]}));
    assert_eq!(states.len(), 64);
    assert!(states.iter().all(|s| s.is_block(blocks::STONE) || s.is_block(blocks::DIRT)));
    assert!(states.iter().any(|s| s.is_block(blocks::STONE)));
    assert!(states.iter().any(|s| s.is_block(blocks::DIRT)));
}

#[test]
fn random_block_accepts_a_single_block() {
    let states = state_of(json!({"type": "minecraft:random_block", "blocks": "minecraft:stone"}));
    assert_eq!(states.len(), 64);
    assert!(states.iter().all(|s| s.is_block(blocks::STONE)));
}

#[test]
fn random_block_accepts_a_block_tag() {
    let states = state_of(json!({"type": "minecraft:random_block", "blocks": "#minecraft:logs"}));
    assert_eq!(states.len(), 64);
    assert!(states.iter().all(|s| s.is(&wafer_data::block_tags::LOGS)));
}

#[test]
fn random_block_with_an_empty_list_has_no_state() {
    assert!(state_of(json!({"type": "minecraft:random_block", "blocks": []})).is_empty());
}

#[test]
fn matching_biomes_tests_the_biome_at_the_position() {
    let mut level = testing::level();
    let test = |json: serde_json::Value, level: &mut crate::server::level::ServerLevel| BlockPredicate::parse(&json).unwrap().test(level, BlockPos::ZERO);
    assert!(test(json!({"type": "minecraft:matching_biomes", "biomes": "minecraft:plains"}), &mut level));
    assert!(test(json!({"type": "minecraft:matching_biomes", "biomes": ["minecraft:desert", "minecraft:plains"]}), &mut level));
    assert!(!test(json!({"type": "minecraft:matching_biomes", "biomes": "minecraft:desert"}), &mut level));
    assert!(!test(json!({"type": "minecraft:matching_biomes", "biomes": "#minecraft:is_ocean"}), &mut level));
    assert!(test(json!({"type": "minecraft:matching_biomes", "biomes": "#minecraft:is_overworld"}), &mut level));
}

#[test]
fn rule_based_provider_uses_matching_biomes() {
    let states = state_of(json!({
        "type": "minecraft:rule_based",
        "rules": [
            {"if_true": {"type": "minecraft:matching_biomes", "biomes": "minecraft:desert"}, "then": {"type": "minecraft:simple", "state": {"id": "minecraft:sand"}}},
            {"if_true": {"type": "minecraft:matching_biomes", "biomes": "minecraft:plains"}, "then": {"type": "minecraft:simple", "state": {"id": "minecraft:stone"}}}
        ]
    }));
    assert_eq!(states.len(), 64);
    assert!(states.iter().all(|s| s.is_block(blocks::STONE)));
}
