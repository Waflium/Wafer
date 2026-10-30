use super::{decode, recording, take_thrown};

fn thrown_while_decoding(registry: &str, json: &str) -> String {
    let element = crate::util::gson::parse_strict(json).unwrap();
    take_thrown();
    let decoded = crate::nbt::with_json_source(|| recording(|| decode(registry, &element)));
    assert!(decoded.is_err(), "{registry}: {json}");
    take_thrown().unwrap_or_else(|| panic!("the decoder throws for {registry}: {json}: {decoded:?}")).print_stack_trace()
}

#[test]
fn data_pack_elements_carry_the_vanilla_codec_frames_of_an_overflowing_number() {
    let cases = [
        ("minecraft:villager_trade", r##"{"wants":{"id":"minecraft:emerald","count":1},"gives":{"id":"minecraft:stick"},"max_uses":1E+10000}"##, include_str!("../dfu_traces/villager_trade_max_uses.txt")),
        ("minecraft:villager_trade", r##"{"wants":{"id":"minecraft:emerald","count":1E+10000},"gives":{"id":"minecraft:stick"}}"##, include_str!("../dfu_traces/villager_trade_wants_count.txt")),
        ("minecraft:trade_set", r##"{"trades":[],"amount":1E+10000}"##, include_str!("../dfu_traces/trade_set_amount.txt")),
        ("minecraft:trial_spawner", r##"{"spawn_range":4,"spawn_potentials":[{"data":{"entity":{"id":"minecraft:zombie"}},"weight":1E+10000}]}"##, include_str!("../dfu_traces/trial_spawner_weight.txt")),
        ("minecraft:trial_spawner", r##"{"spawn_potentials":[{"data":{"entity":{"id":"minecraft:slime","Size":1E+10000}},"weight":1}]}"##, include_str!("../dfu_traces/trial_spawner_entity_size.txt")),
        ("minecraft:trial_spawner", r##"{"ticks_between_spawn":1E+10000}"##, include_str!("../dfu_traces/trial_spawner_ticks_between_spawn.txt")),
        ("minecraft:timeline", r##"{"clock":"minecraft:overworld","tracks":{"minecraft:visual/sky_color":{"keyframes":[{"ticks":1E+10000,"value":"#ffffff"}]}}}"##, include_str!("../dfu_traces/timeline_keyframe_ticks.txt")),
        ("minecraft:worldgen/noise", r##"{"base_octave":1E+10000,"base_amplitude":1}"##, include_str!("../dfu_traces/noise_base_octave.txt")),
        ("minecraft:worldgen/density_function", r##"{"type":"minecraft:gradient","axis":"y","from_coordinate":1E+10000,"to_coordinate":10,"from_value":0,"to_value":1}"##, include_str!("../dfu_traces/density_gradient.txt")),
        ("minecraft:worldgen/density_function", r##"{"type":"minecraft:clamp","input":{"type":"minecraft:gradient","axis":"y","from_coordinate":0,"to_coordinate":1E+10000,"from_value":0,"to_value":1},"min":0,"max":1}"##, include_str!("../dfu_traces/density_clamp_gradient.txt")),
        ("minecraft:worldgen/flat_level_generator_preset", r##"{"display":"minecraft:stone","settings":{"biome":"minecraft:plains","layers":[{"block":"minecraft:stone","height":1E+10000}]}}"##, include_str!("../dfu_traces/flat_preset_height.txt")),
        ("minecraft:worldgen/world_preset", r##"{"dimensions":{"minecraft:overworld":{"type":"minecraft:overworld","generator":{"type":"minecraft:flat","settings":{"biome":"minecraft:plains","layers":[{"block":"minecraft:stone","height":1E+10000}]}}}}}"##, include_str!("../dfu_traces/world_preset_flat_height.txt")),
        ("minecraft:worldgen/biome", r##"{"has_precipitation":true,"temperature":0.5,"downfall":0.5,"effects":{"water_color":4159204},"carvers":[],"features":[],"attributes":{"minecraft:audio/background_music":{"default":{"sound":"minecraft:music.game","min_delay":12000,"max_delay":1E+10000}}}}"##, include_str!("../dfu_traces/biome_music_delay.txt")),
        ("minecraft:worldgen/noise_settings", r##"{"noise":{"min_y":0,"height":1E+10000,"size_horizontal":1,"size_vertical":2}}"##, include_str!("../dfu_traces/noise_settings_height.txt")),
        ("minecraft:worldgen/material_rule", r##"{"type":"minecraft:condition","if_true":{"type":"minecraft:vertical_gradient","random_name":"minecraft:bedrock_floor","true_at_and_below":{"above_bottom":0},"false_at_and_above":{"above_bottom":1E+10000}},"then_run":{"type":"minecraft:block","result_state":{"id":"minecraft:bedrock"}}}"##, include_str!("../dfu_traces/material_rule_gradient.txt")),
        ("minecraft:worldgen/feature", r##"{"type":"minecraft:simple_block","to_place":{"type":"minecraft:weighted","entries":[{"data":{"id":"minecraft:stone"},"weight":1E+10000}]}}"##, include_str!("../dfu_traces/feature_weighted_state_weight.txt")),
        ("minecraft:worldgen/feature", r##"{"type":"minecraft:fossil","fossil_structures":["minecraft:a"],"overlay_structures":["minecraft:b"],"fossil_processors":"minecraft:fossil_rot","overlay_processors":"minecraft:fossil_coal","max_empty_corners_allowed":1E+10000}"##, include_str!("../dfu_traces/feature_fossil_corners.txt")),
        ("minecraft:worldgen/structure", r##"{"type":"minecraft:jigsaw","biomes":"minecraft:plains","step":"surface_structures","spawn_overrides":{},"start_pool":"minecraft:empty","size":1,"start_height":{"absolute":0},"use_expansion_hack":false,"max_distance_from_center":1E+10000}"##, include_str!("../dfu_traces/structure_jigsaw_max_distance.txt")),
        ("minecraft:worldgen/placed_feature", r##"{"feature":"minecraft:ore_coal","placement":[{"type":"minecraft:randomly_selected","placements":[{"type":"minecraft:count","count":1E+10000}]}]}"##, include_str!("../dfu_traces/placed_feature_random_selected.txt")),
        ("minecraft:enchantment", r##"{"anvil_cost":1,"description":"x","effects":{"minecraft:location_changed":[{"effect":{"type":"minecraft:replace_disk","radius":1,"height":1,"offset":[1E+10000,0,0],"block_state":{"id":"minecraft:stone"}}}]},"max_cost":{"base":3,"per_level_above_first":4},"max_level":1,"min_cost":{"base":1,"per_level_above_first":2},"slots":["mainhand"],"supported_items":"#minecraft:swords","weight":5}"##, include_str!("../dfu_traces/enchantment_replace_disk_offset.txt")),
        ("minecraft:enchantment", r##"{"anvil_cost":1,"description":"x","effects":{"minecraft:damage_protection":[{"effect":{"type":"minecraft:add","value":{"type":"minecraft:linear","base":1,"per_level_above_first":1}},"requirements":{"condition":"minecraft:damage_source_properties","predicate":{}}}]},"max_cost":{"base":3,"per_level_above_first":4},"max_level":1E+10000,"min_cost":{"base":1,"per_level_above_first":2},"slots":["mainhand"],"supported_items":"#minecraft:swords","weight":5}"##, include_str!("../dfu_traces/enchantment_damage_protection.txt")),
        ("minecraft:cat_variant", r##"{"asset_id":"minecraft:entity/cat/black","baby_asset_id":"minecraft:entity/cat/black_baby","spawn_conditions":[{"priority":1E+10000}]}"##, include_str!("../dfu_traces/cat_variant_priority.txt")),
        ("minecraft:chicken_variant", r##"{"asset_id":"minecraft:entity/chicken/temperate","baby_asset_id":"minecraft:entity/chicken/temperate_baby","spawn_conditions":[{"priority":1E+10000}]}"##, include_str!("../dfu_traces/chicken_variant_priority.txt")),
        ("minecraft:test_instance", r##"{"type":"minecraft:function","function":"minecraft:always_pass","environment":"minecraft:default","structure":"minecraft:empty","max_ticks":1E+10000}"##, include_str!("../dfu_traces/test_instance_max_ticks.txt")),
    ];
    for (registry, json, expected) in cases {
        assert_eq!(thrown_while_decoding(registry, json), expected, "{registry}: {json}");
    }
}

#[test]
fn a_dimension_type_turns_the_exception_of_its_decoder_into_an_error_message() {
    let json = r##"{"ambient_light":0,"attributes":{},"coordinate_scale":1,"has_ceiling":false,"has_ender_dragon_fight":false,"has_skylight":true,"height":1E+10000,"infiniburn":"#minecraft:infiniburn_overworld","logical_height":384,"min_y":-64,"monster_spawn_block_light_limit":0,"monster_spawn_light_level":7,"timelines":"#minecraft:in_overworld"}"##;
    let element = crate::util::gson::parse_strict(json).unwrap();
    take_thrown();
    let decoded = crate::nbt::with_json_source(|| recording(|| decode("minecraft:dimension_type", &element)));
    assert_eq!(decoded, Err(format!("Caught exception decoding {json}: Number has unsupported scale: 1E+10000")));
    assert!(take_thrown().is_none());
}
