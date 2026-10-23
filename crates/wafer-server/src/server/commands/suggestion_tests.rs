use crate::commands::completion_suggestions;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;

fn server() -> Server {
    testing::server()
}

fn suggest(server: &Server, command: &str) -> (usize, usize, Vec<String>) {
    let source = CommandSourceStack::server(server);
    let suggestions = completion_suggestions(server, &source, command).expect("a suggestion context");
    (suggestions.range.start, suggestions.range.end, suggestions.list.into_iter().map(|s| s.text).collect())
}

#[test]
fn literals_complete_from_the_tree() {
    let server = server();
    assert_eq!(suggest(&server, "/time "), (6, 6, ["add", "of", "pause", "query", "rate", "resume", "set"].map(String::from).to_vec()));
    assert_eq!(suggest(&server, "/time re"), (6, 8, vec!["resume".to_string()]));
}

#[test]
fn time_markers_and_units_come_from_the_server() {
    let server = server();
    let (start, end, markers) = suggest(&server, "/time set ");
    assert_eq!((start, end), (10, 10));
    assert!(markers.contains(&"minecraft:day".to_string()) && markers.contains(&"minecraft:night".to_string()), "{markers:?}");
    assert!(markers.is_sorted_by_key(|m| m.to_lowercase()));
    assert_eq!(suggest(&server, "/time set d"), (10, 11, vec!["minecraft:day".to_string()]));
    assert_eq!(suggest(&server, "/time set 1"), (11, 11, ["d", "s", "t"].map(String::from).to_vec()));
}

#[test]
fn tick_rate_suggests_the_default() {
    let server = server();
    assert_eq!(suggest(&server, "/tick rate "), (11, 11, vec!["20".to_string()]));
}

#[test]
fn score_holders_suggest_selectors() {
    let server = server();
    let selectors = ["@a", "@e", "@n", "@p", "@r", "@s"].map(String::from).to_vec();
    assert_eq!(suggest(&server, "/scoreboard players set "), (24, 24, selectors.clone()));
    assert_eq!(suggest(&server, "/scoreboard players set @"), (24, 25, selectors));
    assert_eq!(suggest(&server, "/scoreboard players set @e"), (26, 26, vec!["[".to_string()]));
    let (_, _, options) = suggest(&server, "/scoreboard players set @e[");
    assert!(!options.contains(&"]".to_string()) && options.contains(&"type=".to_string()) && options.contains(&"limit=".to_string()), "{options:?}");
    assert_eq!(suggest(&server, "/scoreboard players set @e[sort="), (32, 32, ["arbitrary", "furthest", "nearest", "random"].map(String::from).to_vec()));
}

#[test]
fn coordinates_suggest_relative_positions() {
    let server = server();
    assert_eq!(suggest(&server, "/setblock "), (10, 10, ["~", "~ ~", "~ ~ ~"].map(String::from).to_vec()));
    assert_eq!(suggest(&server, "/setblock 1 "), (10, 12, ["1 ~", "1 ~ ~"].map(String::from).to_vec()));
}

fn texts(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

#[test]
fn block_states_suggest_ids_properties_and_values() {
    let server = server();
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ minecraft:oak_st"), (16, 32, texts(&["minecraft:oak_stairs"])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ oak_stairs[facing=north,"), (40, 40, texts(&["half=", "shape=", "waterlogged="])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ oak_stairs["), (27, 27, texts(&["]", "facing=", "half=", "shape=", "waterlogged="])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ oak_stairs[facing="), (34, 34, texts(&["east", "north", "south", "west"])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ oak_stairs[facing=north,half=top,shape=straight,waterlogged=false"), (81, 81, texts(&["]"])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ oak_stairs[facing"), (33, 33, texts(&["="])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ stone["), (22, 22, texts(&["]"])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ wheat[age=3"), (27, 27, texts(&["]"])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ wheat[age="), (26, 26, texts(&["0", "1", "2", "3", "4", "5", "6", "7"])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ red_bed"), (23, 23, texts(&["["])));
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ chest[facing=north]{").2, Vec::<String>::new());
    assert_eq!(suggest(&server, "/setblock ~ ~ ~ #").2, Vec::<String>::new());
}

#[test]
fn integer_property_values_sort_numerically() {
    let server = server();
    let (_, _, values) = suggest(&server, "/setblock ~ ~ ~ snow[layers=");
    assert_eq!(values, texts(&["1", "2", "3", "4", "5", "6", "7", "8"]));
    let (_, _, values) = suggest(&server, "/setblock ~ ~ ~ composter[level=");
    assert_eq!(values, texts(&["0", "1", "2", "3", "4", "5", "6", "7", "8"]));
    let (_, _, values) = suggest(&server, "/setblock ~ ~ ~ redstone_wire[power=");
    assert_eq!(values.len(), 16);
    assert_eq!(values[10], "10");
}

#[test]
fn block_predicates_suggest_tags_and_vague_properties() {
    let server = server();
    assert_eq!(suggest(&server, "/execute if block ~ ~ ~ #minecraft:stairs["), (42, 42, texts(&["]", "facing=", "half=", "shape=", "waterlogged="])));
    assert_eq!(suggest(&server, "/execute if block ~ ~ ~ #minecraft:stairs[facing="), (49, 49, texts(&[",", "]", "east", "north", "south", "west"])));
    assert_eq!(suggest(&server, "/execute if block ~ ~ ~ #minecraft:beds[occupied="), (49, 49, texts(&[",", "]", "false", "true"])));
    assert_eq!(suggest(&server, "/execute if block ~ ~ ~ #minecraft:banners"), (42, 42, texts(&["[", "{"])));
    assert_eq!(suggest(&server, "/execute if block ~ ~ ~ #minecraft:stairs[facing=north,"), (55, 55, texts(&["half=", "shape=", "waterlogged="])));
    let (start, end, tags) = suggest(&server, "/execute if block ~ ~ ~ #minecraft:sta");
    assert_eq!((start, end), (24, 38));
    assert_eq!(tags, texts(&["#minecraft:stairs", "#minecraft:standing_signs"]));
}

#[test]
fn items_suggest_components() {
    let server = server();
    assert_eq!(suggest(&server, "/give @s diamond_sw"), (9, 19, texts(&["minecraft:diamond_sword"])));
    assert_eq!(suggest(&server, "/give @s diamond_sword"), (22, 22, texts(&["["])));
    assert_eq!(suggest(&server, "/give @s diamond_sword[damage=5"), (31, 31, texts(&[",", "]"])));
    assert_eq!(suggest(&server, "/give @s diamond_sword[damage=").2, Vec::<String>::new());
    assert_eq!(
        suggest(&server, "/give @s diamond_sword[!dam"),
        (24, 27, texts(&["minecraft:damage", "minecraft:damage_resistant", "minecraft:damage_type", "minecraft:max_damage"]))
    );
    let (start, _, components) = suggest(&server, "/give @s diamond_sword[");
    assert_eq!(start, 23);
    assert!(components[0] == "!" && components.contains(&"minecraft:damage=".to_string()) && !components.contains(&"minecraft:count=".to_string()), "{components:?}");
    assert_eq!(suggest(&server, "/give @s diamond_sword[damage=5]").2, Vec::<String>::new());
}

#[test]
fn item_predicates_suggest_through_the_packrat_parser() {
    let server = server();
    assert_eq!(suggest(&server, "/clear @s #minecraft:logs"), (25, 25, texts(&["["])));
    assert_eq!(suggest(&server, "/clear @s *[count"), (17, 17, texts(&[",", "=", "]", "|", "~"])));
    assert_eq!(suggest(&server, "/clear @s *[count=").2, Vec::<String>::new());
    assert_eq!(suggest(&server, "/clear @s *[damage~").2, Vec::<String>::new());
    let (start, _, entries) = suggest(&server, "/clear @s *[");
    assert_eq!(start, 12);
    assert!(entries[0] == "!" && entries[1] == "]" && entries.contains(&"minecraft:count".to_string()) && entries.contains(&"minecraft:damage".to_string()), "{entries:?}");
    let (start, _, entries) = suggest(&server, "/clear @s *[!");
    assert_eq!(start, 13);
    assert!(entries.contains(&"minecraft:count".to_string()) && !entries.contains(&"!".to_string()), "{entries:?}");
    let (start, _, entries) = suggest(&server, "/clear @s *[damage=1|!");
    assert_eq!(start, 22);
    assert!(entries.contains(&"minecraft:damage".to_string()), "{entries:?}");
}

#[test]
fn nbt_paths_have_no_suggestions() {
    let server = server();
    assert_eq!(suggest(&server, "/data get entity @s Inv").2, Vec::<String>::new());
    assert_eq!(suggest(&server, "/data get entity @s Inventory[0].").2, Vec::<String>::new());
}

#[test]
fn snbt_arguments_suggest_through_the_packrat_parser() {
    let server = server();
    let numbers = &[".", "b", "B", "d", "D", "e", "E", "f", "F", "i", "I", "l", "L", "s", "S", "u", "U"];
    let cases: &[(&str, usize, &[&str])] = &[
        ("/tellraw @s {", 13, &["\"", "'", "}"]),
        ("/tellraw @s {\"", 14, &["\"", "'", "\\"]),
        ("/tellraw @s {\"a\"", 16, &[":"]),
        ("/tellraw @s {\"a\":1,", 19, &["\"", "'", "}"]),
        ("/tellraw @s {a", 14, &[":"]),
        ("/tellraw @s {a:1b", 17, &[",", "}"]),
        ("/tellraw @s {a:1s", 17, &[",", "b", "B", "i", "I", "l", "L", "s", "S", "}"]),
        ("/tellraw @s {a:1u", 17, &["b", "B", "i", "I", "l", "L", "s", "S"]),
        ("/tellraw @s {a:0x", 17, &["(", "bool", "false", "true", "uuid"]),
        ("/tellraw @s {a:0xF", 18, &[",", "b", "B", "i", "I", "l", "L", "s", "S", "u", "U", "}"]),
        ("/tellraw @s {a:01", 17, &["(", ".", "bool", "d", "D", "e", "E", "f", "F", "false", "true", "uuid"]),
        ("/tellraw @s {a:1.", 17, &[",", "d", "D", "e", "E", "f", "F", "}"]),
        ("/tellraw @s {a:1.5e", 19, &["+", "-"]),
        ("/tellraw @s {a:1.5e5", 20, &[",", "d", "D", "f", "F", "}"]),
        ("/tellraw @s {a:-", 16, &["(", ".", "0", "bool", "false", "true", "uuid"]),
        ("/tellraw @s {a:[", 16, &["]", "B", "I", "L"]),
        ("/tellraw @s {a:[1,", 18, &["]"]),
        ("/tellraw @s {a:[B", 17, &["(", ",", ";", "]"]),
        ("/tellraw @s {a:[B;", 18, &["+", "-", "0", "]"]),
        ("/tellraw @s {a:[B;1", 19, &[",", "]", "b", "B", "i", "I", "l", "L", "s", "S", "u", "U"]),
        ("/tellraw @s {a:[B;1b", 20, &[",", "]"]),
        ("/tellraw @s {a:[X", 17, &["(", ",", "]"]),
        ("/tellraw @s {a:bool", 19, &["(", ",", "}"]),
        ("/tellraw @s {a:bool(", 20, &[")"]),
        ("/tellraw @s {a:bool(1)", 22, &[",", "}"]),
        ("/tellraw @s {a:uuid(\"", 21, &["\"", "'", "\\"]),
        ("/tellraw @s {a:foo(1,", 21, &[")"]),
        ("/tellraw @s {a:tru", 18, &["(", ",", "}"]),
        ("/tellraw @s {a:\"x\\", 18, &["\"", "'", "\\", "b", "f", "n", "N", "r", "s", "t", "u", "U", "x"]),
        ("/tellraw @s {a:\"x\\N", 19, &["{"]),
        ("/tellraw @s {a:\"x\\N{a", 21, &["}"]),
        ("/tellraw @s {a:'say \"", 21, &["\"", "'", "\\"]),
        ("/tellraw @s {\"\":1", 17, &[".", "b", "B", "d", "D", "e", "E", "f", "F", "i", "I", "l", "L", "s", "S", "u", "U"]),
        ("/tellraw @s {a:{b:1},c", 22, &[":"]),
        ("/tellraw @s {a:-abc", 19, &["(", "bool", "false", "true", "uuid"]),
        ("/tellraw @s {a:1.5fx", 0, &[]),
        ("/tellraw @s {a:0x1uL", 20, &[",", "}"]),
        ("/tellraw @s {a:9999999999", 25, numbers),
        ("/tellraw @s [", 13, &["]", "B", "I", "L"]),
        ("/tellraw @s [B;1,2,3]", 0, &[]),
        ("/tellraw @s [ B ;", 17, &["+", "-", "0", "]"]),
        ("/tellraw @s tr", 14, &["("]),
        ("/tellraw @s 0", 13, &[".", "b", "B", "d", "D", "e", "E", "f", "F", "i", "I", "l", "L", "s", "S", "u", "U", "x", "X"]),
        ("/tellraw @s 1.0", 15, &["d", "D", "e", "E", "f", "F"]),
        ("/tellraw @s 1b", 0, &[]),
        ("/tellraw @s -x", 14, &["(", "bool", "false", "true", "uuid"]),
        ("/tellraw @s \"abc\\", 17, &["\"", "'", "\\", "b", "f", "n", "N", "r", "s", "t", "u", "U", "x"]),
        ("/tellraw @s {}", 0, &[]),
        ("/tellraw @s ", 0, &[]),
        ("/title @s title {", 17, &["\"", "'", "}"]),
        ("/title @s title {a:bool(", 24, &[")"]),
        ("/title @s subtitle [", 20, &["]", "B", "I", "L"]),
        ("/data modify entity @s Inventory append value {a:[B;", 52, &["+", "-", "0", "]"]),
        ("/data modify storage a:b c set value {\"", 39, &["\"", "'", "\\"]),
        ("/data modify block ~ ~ ~ Items merge value {a:1", 47, &[",", ".", "b", "B", "d", "D", "e", "E", "f", "F", "i", "I", "l", "L", "s", "S", "u", "U", "}"]),
        ("/scoreboard players display numberformat @s o styled {", 54, &["\"", "'", "}"]),
        ("/bossbar set a:b name {a:1", 26, &[",", ".", "b", "B", "d", "D", "e", "E", "f", "F", "i", "I", "l", "L", "s", "S", "u", "U", "}"]),
        ("/data merge entity @s {", 0, &[]),
        ("/data get entity @s [", 0, &[]),
        ("/summon pig ~ ~ ~ {\"", 0, &[]),
        ("/execute if data storage a:b c {a:1", 0, &[]),
    ];
    for &(command, start, expected) in cases {
        assert_eq!(suggest(&server, command), (start, start, texts(expected)), "{command}");
    }
}

#[test]
fn heightmaps_suggest_the_kept_types() {
    let server = server();
    assert_eq!(suggest(&server, "/execute positioned over "), (25, 25, ["motion_blocking", "motion_blocking_no_leaves", "ocean_floor", "world_surface"].map(String::from).to_vec()));
}

#[test]
fn whitelist_add_suggests_online_players_missing_from_the_list() {
    let mut server = server();
    testing::server_player_with_profile(&mut server, crate::network::login::Profile { uuid: 1, name: "Alice".into(), properties: Vec::new() }, wafer_core::Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    assert_eq!(suggest(&server, "/whitelist add "), (15, 15, vec!["Alice".to_string()]));
}

#[test]
fn game_rules_behind_disabled_features_are_not_suggested() {
    let server = server();
    let (_, _, rules) = suggest(&server, "/gamerule ");
    assert!(!rules.iter().any(|rule| rule.ends_with("max_minecart_speed")));
    assert_eq!(rules.len(), 116);
}
