use wafer_protocol::nbt::Nbt;

use super::tests_from_1022::escape_lone_surrogates;
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::world::level::chunk::storage::{decode_nbt, encode_nbt};

const CASES: &str = include_str!("tests_java_parity_cases.tsv");

fn update_value(ty: &'static str, from: i32, to: i32, input: &str) -> Nbt {
    let input = crate::nbt::parse_fully(input).expect("valid snbt");
    get_data_fixer().update(TypeReference(ty), Dynamic::new(input), from, to).into_value().expect("a value")
}

fn update_catching(ty: &'static str, from: i32, to: i32, input: &str) -> String {
    match crate::util::catch_exception(|| escape_lone_surrogates(&crate::nbt::to_string(&update_value(ty, from, to, input)))) {
        Ok(output) => output,
        Err(message) => format!("ERR {message}"),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap()).collect()
}

#[test]
fn updates_match_the_vanilla_oracle_in_the_edge_cases() {
    for line in CASES.lines() {
        let fields: Vec<&str> = line.splitn(5, '\t').collect();
        let ty: &'static str = Box::leak(fields[0].to_string().into_boxed_str());
        let (from, to): (i32, i32) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
        let actual = update_catching(ty, from, to, fields[3]);
        let matches = match (actual.strip_prefix("ERR "), fields[4].strip_prefix("ERR ")) {
            (Some(message), Some(vanilla)) => vanilla.contains(message),
            (None, None) => actual == fields[4],
            _ => false,
        };
        assert!(matches, "{ty} {from} -> {to}: {}\n  vanilla: {}\n  wafer:   {actual}", fields[3], fields[4]);
    }
}

#[test]
fn shulker_boxes_with_block_entity_tags_throw_like_vanilla() {
    for input in [
        r#"{id:"minecraft:shulker_box",Count:1b,Damage:0s,tag:{BlockEntityTag:{Color:3}}}"#,
        r#"{id:"minecraft:shulker_box",Count:1b,Damage:0s,tag:{BlockEntityTag:{}}}"#,
        r#"{id:"minecraft:shulker_box",Count:1b,Damage:0s,tag:{BlockEntityTag:{Color:-1}}}"#,
    ] {
        assert_eq!(update_catching("item_stack", 812, 813, input), "ERR java.lang.IllegalArgumentException: Couldn't upcast", "{input}");
    }
    assert_eq!(
        update_catching("player", 812, 5023, r#"{Inventory:[{id:"minecraft:shulker_box",Count:1b,Damage:0s,Slot:0b,tag:{BlockEntityTag:{Color:3}}}]}"#),
        "ERR java.lang.IllegalArgumentException: Couldn't upcast"
    );
}

const END_TAG_LEVELS: &[(&str, &str, &str, &str)] = &[
    (
        r#"{generatorName:"buffet",generatorOptions:""}"#,
        r#"{generatorName:"buffet",generatorOptions:END}"#,
        "0a00000008000d67656e657261746f724e616d65000662756666657400",
        "{}",
    ),
    (
        r#"{generatorName:"buffet",generatorOptions:" ",b:2,c:3,d:4}"#,
        r#"{b:2,c:3,d:4,generatorName:"buffet",generatorOptions:END}"#,
        "0a00000008000d67656e657261746f724e616d65000662756666657403000162000000020300016300000003030001640000000400",
        "{}",
    ),
    (
        r#"{generatorName:"buffet",generatorOptions:"null",Player:{Pos:[1.0d]}}"#,
        r#"{Player:{Pos:[1.0d]},generatorName:"buffet",generatorOptions:END}"#,
        "0a00000008000d67656e657261746f724e616d6500066275666665740a0006506c61796572090003506f7306000000013ff00000000000000000",
        "{}",
    ),
    (
        r#"{generatorName:"buffet",generatorOptions:"",C:1,D:2,E:3,F:4,G:5,H:6,I:7,J:8,K:9,L:10}"#,
        r#"{C:1,D:2,E:3,F:4,G:5,H:6,I:7,J:8,K:9,L:10,generatorName:"buffet",generatorOptions:END}"#,
        "0a00000008000d67656e657261746f724e616d65000662756666657403000143000000010300014400000002030001450000000303000146000000040300014700000005030001480000000603000149000000070300014a000000080300014b000000090300014c0000000a00",
        "{}",
    ),
];

#[test]
fn buffet_options_that_parse_to_null_write_an_end_tag_like_vanilla() {
    for (input, snbt, expected_hex, loaded) in END_TAG_LEVELS {
        let level = update_value("level", 1505, 1506, input);
        assert_eq!(crate::nbt::to_string(&level), *snbt);
        let bytes = encode_nbt(&level, &[]);
        assert_eq!(hex(&bytes), *expected_hex, "{input}");
        assert_eq!(crate::nbt::to_string(&decode_nbt(&bytes).unwrap()), *loaded, "{input}");
    }
}

#[test]
fn an_end_tag_in_a_nested_compound_ends_the_parents_when_written() {
    let level = update_value("level", 1505, 5023, r#"{generatorName:"buffet",generatorOptions:"",LevelName:"w",RandomSeed:5L,A:1,B:2,C:3,D:4,E:5,F:6,G:7}"#);
    assert!(crate::nbt::to_string(&level).ends_with(r#"world_gen_settings:{RandomSeed:5L,generatorName:"buffet",generatorOptions:END}}"#));
    let loaded = decode_nbt(&encode_nbt(&level, &[])).unwrap();
    assert_eq!(
        crate::nbt::to_string(&loaded),
        r#"{A:1,B:2,C:3,D:4,E:5,F:6,G:7,RandomSeed:5L,difficulty_settings:{difficulty:"normal",hardcore:0b,locked:0b},generatorName:"buffet",scheduled_events:{events:[]},spawn:{dimension:"minecraft:overworld",pitch:0.0f,pos:[I;0,0,0],yaw:0.0f},wandering_trader_migration_data:{spawn_chance:25,spawn_delay:24000},weather_data:{clear_weather_time:0,rain_time:0,raining:0b,thunder_time:0,thundering:0b},world_gen_settings:{}}"#
    );
}

#[test]
fn sections_with_the_same_index_throw_with_vanilla_s_message() {
    let section = |name: &str| format!(r#"{{Y:0b,Palette:[{{Name:"{name}"}}]}}"#);
    let input = format!(r#"{{Level:{{xPos:0,zPos:0,Sections:[{},{}]}}}}"#, section("minecraft:air"), section("minecraft:stone"));
    let actual = update_catching("chunk", 1495, 1496, &input);
    let prefix = "ERR java.lang.IllegalStateException: Duplicate key 0 (attempted merging values net.minecraft.util.datafix.fixes.LeavesFix$LeavesSection@";
    let rest = actual.strip_prefix(prefix).unwrap_or_else(|| panic!("{actual}"));
    let (first, rest) = rest.split_once(" and net.minecraft.util.datafix.fixes.LeavesFix$LeavesSection@").expect("two sections");
    let second = rest.strip_suffix(')').expect("a closing parenthesis");
    for hash in [first, second] {
        assert!(!hash.is_empty() && hash.len() <= 8 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) && i32::from_str_radix(hash, 16).unwrap() > 0, "{hash}");
    }
    assert_ne!(first, second);
}

#[test]
fn updating_to_a_version_with_sub_version_schemas_returns_the_input_when_the_type_changes() {
    let chunk = r#"{Level:{xPos:2,zPos:3,Sections:[{Y:0b,Blocks:[B;1B,2B,3B],Data:[B;1B]}]}}"#;
    assert_eq!(update_catching("chunk", 1450, 1451, chunk), r#"{Level:{Sections:[{Blocks:[B;1B,2B,3B],Data:[B;1B],Y:0b}],xPos:2,zPos:3}}"#);
    assert_eq!(update_catching("chunk", 1450, 1452, chunk), "ERR ChunkNibbleArrays should be 2048 bytes not: 1");
}

#[test]
fn decoded_end_tags_are_empty_dynamics() {
    let level = update_value("level", 1505, 1506, r#"{generatorName:"buffet",generatorOptions:""}"#);
    let dynamic = Dynamic::new(level);
    assert_eq!(dynamic.get("generatorOptions").result().unwrap().get_value(), None);
    assert!(dynamic.get("generatorOptions").result().is_some());
}

const SURROGATE_ENTITY_HEX: &str = "0a00000a000a437573746f6d4e616d6508000474657874000b61eda0bdedb88020eda080000800026964000d6d696e6563726166743a70696700";

#[test]
fn lone_surrogates_in_text_components_are_written_like_vanilla() {
    let entity = update_value("entity", 3817, 5023, r#"{id:"minecraft:pig",CustomName:'{"text":"a\\ud83d\\ude00 \\ud800"}'}"#);
    assert_eq!(escape_lone_surrogates(&crate::nbt::to_string(&entity)), "{CustomName:{text:\"a\u{1F600} \\ud800\"},id:\"minecraft:pig\"}");
    assert_eq!(hex(&encode_nbt(&entity, &[])), SURROGATE_ENTITY_HEX);
}

#[test]
fn lone_surrogates_are_read_back_from_nbt() {
    let bytes = unhex(SURROGATE_ENTITY_HEX);
    let entity = decode_nbt(&bytes).unwrap();
    assert_eq!(escape_lone_surrogates(&crate::nbt::to_string(&entity)), "{CustomName:{text:\"a\u{1F600} \\ud800\"},id:\"minecraft:pig\"}");
    assert_eq!(encode_nbt(&entity, &[]), bytes);
}
