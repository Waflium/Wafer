use serde_json::Value;

use super::structure_processor::StructureProcessorListHolder;
use super::structure_template_manager;

#[test]
fn every_processor_list_parses() {
    for (name, _) in wafer_data::PROCESSOR_LISTS {
        StructureProcessorListHolder::parse(&Value::String((*name).to_owned())).unwrap().value();
    }
}

#[test]
fn every_structure_template_loads() {
    let templates = structure_template_manager::list_templates();
    assert_eq!(templates.len(), crate::world::item::crafting::tests::vanilla_data_files_with_extension("structure", ".nbt"));
    for name in templates {
        let size = structure_template_manager::get_or_create(&name).get_size();
        assert!(size.x >= 1 && size.y >= 1 && size.z >= 1, "{name} has size {size:?}");
    }
}

mod codecs {
    use crate::core::data_registries::take_thrown;
    use crate::world::item::component::json_to_nbt;
    use crate::world::item::component::types::Pair;
    use crate::world::level::levelgen::structure::pools::structure_template_pool;
    use crate::world::level::levelgen::structure::templatesystem::structure_processor;

    fn decode(codec: Pair, json: &str) -> Result<(), String> {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        crate::core::data_registries::json_entries("minecraft:worldgen/template_pool");
        take_thrown();
        let result = crate::nbt::with_json_source(|| codec.0.decode(&json_to_nbt(&value)).map(drop));
        match take_thrown() {
            Some(thrown) => Err(thrown.description),
            None => result,
        }
    }

    fn pool(json: &str) -> Result<(), String> {
        decode(structure_template_pool::direct_codec(), json)
    }

    fn processors(json: &str) -> Result<(), String> {
        decode(structure_processor::list_direct_codec(), json)
    }

    fn rule(extra: &str) -> String {
        format!(
            r#"[{{"processor_type":"minecraft:rule","rules":[{{"input_predicate":{{"predicate_type":"minecraft:always_true"}},"location_predicate":{{"predicate_type":"minecraft:always_true"}},"output_state":"minecraft:stone"{extra}}}]}}]"#
        )
    }

    fn weighted(weight: &str) -> String {
        format!(r#"{{"fallback":"minecraft:empty","elements":[{{"weight":{weight},"element":{{"element_type":"minecraft:empty_pool_element"}}}}]}}"#)
    }

    #[test]
    fn pool_weights_are_limited_to_one_through_one_hundred_fifty() {
        assert_eq!(pool(&weighted("150")), Ok(()));
        assert_eq!(pool(&weighted("0")), Err("Value 0 outside of range [1:150]".into()));
        assert_eq!(pool(&weighted("151")), Err("Value 151 outside of range [1:150]".into()));
        assert_eq!(pool(&weighted("\"x\"")), Err("Not a number: \"x\"".into()));
        assert_eq!(pool(&weighted("1e10")), Err("Value 1410065408 outside of range [1:150]".into()));
    }

    #[test]
    fn pool_element_and_weight_are_read_like_a_pair_codec() {
        let pool_of = |element: &str| format!(r#"{{"fallback":"minecraft:empty","elements":[{element}]}}"#);
        assert_eq!(pool(&pool_of("{}")), Err("No key element in MapLike[{}]".into()));
        assert_eq!(pool(&pool_of(r#"{"weight":0}"#)), Err(r#"No key element in MapLike[{"weight":0}]"#.into()));
        assert_eq!(
            pool(&pool_of(r#"{"weight":0,"element":{"element_type":"minecraft:nope"}}"#)),
            Err("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/structure_pool_element]: minecraft:nope".into())
        );
        assert_eq!(pool(&pool_of(r#"5,"s",null,[]"#)), Err(r#"Not a JSON object: []; Not a JSON object: null; Not a JSON object: "s"; Not a JSON object: 5"#.into()));
    }

    #[test]
    fn a_list_pool_element_without_elements_throws() {
        let list = r#"{"fallback":"minecraft:empty","elements":[{"weight":1,"element":{"element_type":"minecraft:list_pool_element","elements":[],"projection":"rigid"}}]}"#;
        assert_eq!(pool(list), Err("java.lang.IllegalArgumentException: Elements are empty".into()));
        let partial = list.replace("[]", r#"[5]"#);
        assert_eq!(pool(&partial), Err("java.lang.IllegalArgumentException: Elements are empty".into()));
        let without_projection = list.replace(r#","projection":"rigid""#, "");
        assert_eq!(pool(&without_projection), Err(r#"No key projection in MapLike[{"element_type":"minecraft:list_pool_element","elements":[]}]"#.into()));
    }

    #[test]
    fn an_inline_fallback_is_decoded_when_the_string_is_not_an_identifier() {
        assert_eq!(pool(r#"{"fallback":"Minecraft:Stone","elements":[]}"#), Err(r#"Not a JSON object: "Minecraft:Stone""#.into()));
        assert_eq!(pool(r#"{"fallback":{"fallback":"minecraft:empty","elements":[]},"elements":[]}"#), Ok(()));
    }

    #[test]
    fn processor_lists_accept_an_object_or_a_bare_list() {
        assert_eq!(processors(r#"{"processors":[{"processor_type":"minecraft:nop"}]}"#), Ok(()));
        assert_eq!(processors(r#"[{"processor_type":"minecraft:nop"}]"#), Ok(()));
        assert_eq!(
            processors(r#"{"processors":5}"#),
            Err(r#"Failed to parse either. First: Not a json array: 5; Second: Not a json array: {"processors":5}"#.into())
        );
        assert_eq!(
            processors(r#"[{"processor_type":"minecraft:nope"}]"#),
            Err("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/structure_processor]: minecraft:nope".into())
        );
    }

    #[test]
    fn block_rot_integrity_uses_java_ordering_for_negative_zero() {
        assert_eq!(processors(r#"[{"processor_type":"minecraft:block_rot","integrity":-0.0}]"#), Err("Value -0.0 outside of range [0.0:1.0]".into()));
        assert_eq!(processors(r#"[{"processor_type":"minecraft:block_rot","integrity":1.5}]"#), Err("Value 1.5 outside of range [0.0:1.0]".into()));
        assert_eq!(processors(r#"[{"processor_type":"minecraft:block_age","mossiness":1e39}]"#), Ok(()));
    }

    #[test]
    fn a_tag_with_an_invalid_identifier_reports_both_alternatives() {
        assert_eq!(
            processors(r##"[{"processor_type":"minecraft:block_rot","integrity":0.5,"rottable_blocks":"#Minecraft:logs"}]"##),
            Err("Failed to parse either. First: Not a valid resource location: Minecraft:logs Non [a-z0-9_.-] character in namespace of identifier: Minecraft:logs; Second: Failed to parse either. First: Not a json array: \"#Minecraft:logs\"; Second: Not a valid resource location: #Minecraft:logs Non [a-z0-9_.-] character in namespace of identifier: #Minecraft:logs".into())
        );
    }

    #[test]
    fn a_block_state_with_a_null_id_is_missing_its_id() {
        let ignore = r#"[{"processor_type":"minecraft:block_ignore","blocks":[{"id":null,"properties":{"lit":"true"}}]}]"#;
        assert_eq!(
            processors(ignore),
            Err(r#"Failed to parse either. First: Not a string: {"id":null,"properties":{"lit":"true"}}; Second: No key id in MapLike[{"id":null,"properties":{"lit":"true"}}]"#.into())
        );
    }

    #[test]
    fn capped_limit_must_be_positive() {
        let capped = |limit: &str| format!(r#"[{{"processor_type":"minecraft:capped","delegate":{{"processor_type":"minecraft:nop"}},"limit":{limit}}}]"#);
        assert_eq!(processors(&capped("3")), Ok(()));
        assert_eq!(processors(&capped("0")), Err("Value provider too low: 1 [0-0]".into()));
        assert_eq!(processors(&capped(r#"{"type":"minecraft:uniform","min_inclusive":0,"max_inclusive":3}"#)), Err("Value provider too low: 1 [0-3]".into()));
    }

    #[test]
    fn a_linear_position_test_with_an_empty_range_throws_even_inside_a_lenient_field() {
        let position = |test: &str| rule(&format!(r#","position_predicate":{test}"#));
        assert_eq!(
            processors(&position(r#"{"predicate_type":"minecraft:linear_pos"}"#)),
            Err("java.lang.IllegalArgumentException: Invalid range: [0,0]".into())
        );
        assert_eq!(
            processors(&position(r#"{"predicate_type":"minecraft:axis_aligned_linear_pos","min_dist":5,"max_dist":1,"axis":"q"}"#)),
            Err("java.lang.IllegalArgumentException: Invalid range: [5,1]".into())
        );
        assert_eq!(processors(&position(r#"{"predicate_type":"minecraft:linear_pos","min_dist":0,"max_dist":1}"#)), Ok(()));
        assert_eq!(processors(&position(r#"{"predicate_type":"minecraft:nope"}"#)), Ok(()));
    }

    #[test]
    fn errors_in_lenient_rule_fields_are_swallowed() {
        assert_eq!(processors(&rule(r#","block_entity_modifier":{"type":"minecraft:append_static","data":"x"}"#)), Ok(()));
        assert_eq!(processors(&rule(r#","block_entity_modifier":{"type":"minecraft:append_static","data":{"a":1}}"#)), Ok(()));
    }

    #[test]
    fn append_static_data_with_a_null_throws_a_null_pointer_exception() {
        let thrown = r#"java.lang.NullPointerException: Cannot invoke "com.google.gson.JsonElement.getAsJsonPrimitive()" because "input" is null"#;
        assert_eq!(processors(&rule(r#","block_entity_modifier":{"type":"minecraft:append_static","data":{"a":null}}"#)), Err(thrown.into()));
        assert_eq!(processors(&rule(r#","block_entity_modifier":{"type":"minecraft:append_static","data":[null]}"#)), Err(thrown.into()));
    }

    #[test]
    fn rule_tests_report_their_own_errors() {
        let input = |test: &str| format!(r#"[{{"processor_type":"minecraft:rule","rules":[{{"input_predicate":{test},"location_predicate":{{"predicate_type":"minecraft:always_true"}},"output_state":"minecraft:stone"}}]}}]"#);
        assert_eq!(
            processors(&input(r#"{"predicate_type":"minecraft:block_match","block":"minecraft:nope"}"#)),
            Err("Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: minecraft:nope".into())
        );
        assert_eq!(processors(&input(r#"{"predicate_type":"minecraft:random_block_match","block":"minecraft:stone"}"#)), Err(r#"No key probability in MapLike[{"predicate_type":"minecraft:random_block_match","block":"minecraft:stone"}]"#.into()));
        assert_eq!(
            processors(&input(r#"{"predicate_type":"minecraft:not","rule":{"predicate_type":"minecraft:nope"}}"#)),
            Err("Unknown registry key in ResourceKey[minecraft:root / minecraft:rule_test]: minecraft:nope".into())
        );
        assert_eq!(processors(&input(r#"{"predicate_type":"minecraft:height_match","min_inclusive":0,"max_inclusive":10}"#)), Ok(()));
    }
}

mod template_entities {
    use wafer_core::ChunkPos;
    use wafer_protocol::nbt::Nbt;

    use crate::server::level::testing;
    use crate::world::entity::{LevelEntity, sound_event};
    use crate::world::level::levelgen::structure::templatesystem::structure_template::create_entity_ignore_exception;

    const WOLF_TARGET: u128 = 0x0000_0001_0000_0002_0000_0003_0000_0004;

    fn wolf_tag() -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String("minecraft:wolf".into())),
            ("AngerTime".into(), Nbt::Int(100)),
            ("angry_at".into(), Nbt::IntArray(vec![1, 2, 3, 4])),
        ])
    }

    fn item_frame_tag() -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String("minecraft:item_frame".into())),
            ("Item".into(), Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:stone".into())), ("count".into(), Nbt::Int(1))])),
        ])
    }

    fn anger(entity: &LevelEntity) -> (i64, Option<u128>) {
        let LevelEntity::Mob(mob) = entity else { unreachable!("a wolf") };
        let neutral_mob = mob.neutral_mob().expect("a neutral mob");
        (neutral_mob.get_persistent_anger_end_time(&mob.entity), neutral_mob.get_persistent_anger_target().map(|target| target.get_uuid()))
    }

    #[test]
    fn legacy_anger_time_and_target_are_applied_in_a_level() {
        let mut level = testing::level();
        level.game_time = 40;
        let entity = create_entity_ignore_exception(&mut level, &wolf_tag()).expect("a wolf");
        assert_eq!(anger(&entity), (140, Some(WOLF_TARGET)));
    }

    #[test]
    fn legacy_anger_time_and_target_are_applied_in_a_region() {
        let mut level = testing::level();
        level.chunks.region_level_data.game_time = 40;
        let entity = level.chunks.with_region(ChunkPos::new(0, 0), |region, _| create_entity_ignore_exception(region, &wolf_tag())).expect("a wolf");
        assert_eq!(anger(&entity), (140, Some(WOLF_TARGET)));
    }

    #[test]
    fn framed_items_play_their_sound_in_a_region() {
        let mut level = testing::level();
        let entity = level.chunks.with_region(ChunkPos::new(0, 0), |region, _| create_entity_ignore_exception(region, &item_frame_tag()));
        assert!(entity.is_some());
        let sounds = level.chunks.take_world_gen_entity_sounds();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].1, sound_event("minecraft:entity.item_frame.add_item"));
    }
}
