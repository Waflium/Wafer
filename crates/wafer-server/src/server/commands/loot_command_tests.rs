use crate::commands::completion_suggestions;
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn flat_server() -> Server {
    let mut server = testing::server();
    run(&mut server, "forceload add -32 -32 32 32");
    server
}

#[test]
fn loot_command_matches_vanilla() {
    let mut server = flat_server();
    for (x, block) in [(1, "barrel"), (2, "furnace"), (3, "jukebox"), (8, "lectern")] {
        run(&mut server, &format!("setblock {x} -60 0 minecraft:{block}"));
    }
    assert_eq!(run(&mut server, "loot replace block 1 -60 0 container.27 mine 0 -61 0"), ["The target does not have slot 27"]);
    assert_eq!(run(&mut server, "loot replace block 1 -60 0 container.25 5 mine 0 -61 0"), ["An unexpected error occurred while trying to execute that command"]);
    assert_eq!(run(&mut server, "data get block 1 -60 0 Items"), ["1, -60, 0 has the following block data: [{count: 1, Slot: 25b, id: \"minecraft:dirt\"}]"]);
    assert_eq!(run(&mut server, "loot replace block 1 -60 0 weapon.* mine 0 -61 0"), ["Only single slots allowed: got 'weapon.*'", "...0 weapon.* mine 0 -61 0<--[HERE]"]);
    assert_eq!(run(&mut server, "loot replace block 1 -60 0 nope mine 0 -61 0"), ["Unknown slot 'nope'", "...-60 0 nope mine 0 -61 0<--[HERE]"]);
    assert_eq!(run(&mut server, "loot insert 2 -60 0 mine 0 -61 0"), ["Dropped 1 [Dirt] from loot table minecraft:blocks/grass_block"]);
    assert_eq!(run(&mut server, "loot insert 2 -60 0 mine 0 -61 0"), ["Dropped 1 [Dirt] from loot table minecraft:blocks/grass_block"]);
    assert_eq!(run(&mut server, "data get block 2 -60 0 Items"), ["2, -60, 0 has the following block data: [{count: 2, Slot: 0b, id: \"minecraft:dirt\"}]"]);
    assert_eq!(run(&mut server, "loot insert 3 -60 0 loot {pools:[{rolls:1,entries:[{type:\"minecraft:item\",name:\"minecraft:music_disc_cat\"}]}]}"), ["Dropped 1 [Music Disc]"]);
    assert_eq!(run(&mut server, "loot insert 8 -60 0 mine 0 -61 0"), ["Target position 8, -60, 0 is not a container"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 mine 0 -64 0"), ["Block Bedrock has no loot table"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 mine 0 -60 5"), ["Block Air has no loot table"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 mine 0 -61 0 mainhand"), ["An entity is required to run this command here"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 mine 99999 -61 0"), ["That position is not loaded"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 mine 0 -61 0 diamond_pickaxe[enchantments={silk_touch:1}]"), ["Dropped 1 [Grass Block] from loot table minecraft:blocks/grass_block"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 loot {pools:[{rolls:2,entries:[{type:\"minecraft:item\",name:\"minecraft:apple\"}]}]}"), ["Dropped 2 items"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 loot foo:bar"), ["Can't find element 'foo:bar' in registry 'minecraft:loot_table'", "...ot foo:bar<--[HERE]"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 loot {pools:1}"), ["Failed to parse structure: Not a list: 1", "... {pools:1}<--[HERE]"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 loot {}"), ["Dropped 0 items"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 loot Foo"), ["Failed to parse structure: Not a map: \"Foo\"", "...0 loot Foo<--[HERE]"]);
    assert_eq!(run(&mut server, "loot insert 1 -60 0 loot minecraft:blocks/stone"), ["Dropped 1 [Cobblestone]"]);
    assert_eq!(run(&mut server, "loot give @a loot minecraft:blocks/stone"), ["No player was found"]);
    run(&mut server, "summon chicken 3 -60 3 {NoAI:1b,Tags:[\"c\"]}");
    run(&mut server, "summon chicken 4 -60 3 {NoAI:1b,Tags:[\"c\"]}");
    run(&mut server, "summon item 5 -60 5 {NoGravity:1b,Item:{id:\"minecraft:dirt\",count:1}}");
    assert_eq!(run(&mut server, "loot insert 1 -60 0 kill @e[type=item,limit=1]"), ["Entity Dirt has no loot table"]);
    assert_eq!(run(&mut server, "loot replace entity @e[tag=c] armor.head 3 mine 0 -61 0"), ["Dropped 4 items from loot table minecraft:blocks/grass_block"]);
    assert_eq!(run(&mut server, "loot replace entity @e[tag=c] armor.feet mine 0 -61 0"), ["Dropped 0 items from loot table minecraft:blocks/grass_block"]);
    assert_eq!(run(&mut server, "execute as @e[tag=c,limit=1] run loot replace entity @s weapon.offhand mine 0 -61 0 mainhand"), ["Dropped 1 [Dirt] from loot table minecraft:blocks/grass_block"]);
    assert_eq!(run(&mut server, "data get entity @e[tag=c,limit=1] equipment"), ["Chicken has the following entity data: {head: {count: 1, id: \"minecraft:dirt\"}, offhand: {count: 1, id: \"minecraft:dirt\"}}"]);
}

#[test]
fn item_command_matches_vanilla() {
    let mut server = flat_server();
    for (x, block) in [(0, "chest"), (1, "barrel"), (7, "shulker_box")] {
        run(&mut server, &format!("setblock {x} -60 0 minecraft:{block}"));
    }
    assert_eq!(run(&mut server, "item replace block 0 -60 0 container.0 with diamond 5"), ["Replaced 1 slot(s) at 0, -60, 0 with [Diamond]"]);
    assert_eq!(run(&mut server, "item replace block 0 -60 0 container.* with stone"), ["Replaced 1 slot(s) at 0, -60, 0 with [Stone]"]);
    assert_eq!(run(&mut server, "item replace block 0 -60 0 container.4 with stone 100"), ["Integer must not be more than 99: found 100", "...ith stone 100<--[HERE]"]);
    assert_eq!(run(&mut server, "item replace block 0 -60 0 container.4 with stone 99"), ["minecraft:stone can only stack up to 64"]);
    assert_eq!(run(&mut server, "item fill block 1 -60 0 container.* with stone 2"), ["Replaced 27 slot(s) at 1, -60, 0 with [Stone]"]);
    assert_eq!(run(&mut server, "item override block 1 -60 0 hotbar.* with apple"), ["Replaced 9 slot(s) at 1, -60, 0 with [Apple]"]);
    assert_eq!(run(&mut server, "item replace block 1 -60 0 container.* from block 0 -60 0 container.*"), ["Replaced 27 slot(s) at 1, -60, 0"]);
    assert_eq!(run(&mut server, "data get block 1 -60 0 Items"), ["1, -60, 0 has the following block data: [{count: 1, Slot: 0b, id: \"minecraft:stone\"}]"]);
    assert_eq!(run(&mut server, "item fill block 1 -60 0 inventory.* from block 0 -60 0 hotbar.*"), ["Replaced 18 slot(s) at 1, -60, 0"]);
    assert_eq!(run(&mut server, "item replace block 1 -60 0 weapon from block 0 -60 0 container.0"), ["The target does not have slot weapon"]);
    assert_eq!(run(&mut server, "item replace block 1 -60 0 container.0 from block 0 -60 0 weapon"), ["The source does not have slot weapon"]);
    assert_eq!(run(&mut server, "item replace block 1 -60 0 container.0 from block 0 -60 0 {type:\"minecraft:empty\"}"), ["The source does not have specified slots"]);
    assert_eq!(run(&mut server, "item replace block 1 -60 0 container.0 from block 0 -61 0 container.0"), ["Source position 0, -61, 0 is not a container"]);
    assert_eq!(run(&mut server, "item replace block 0 -61 0 container.0 with stone"), ["Target position 0, -61, 0 is not a container"]);
    assert_eq!(run(&mut server, "item modify block 1 -60 0 container.* {type:\"minecraft:set_count\",count:7}"), ["Modified 3 slot(s) at 1, -60, 0"]);
    assert_eq!(run(&mut server, "item modify block 1 -60 0 container.26 {type:\"minecraft:set_count\",count:7}"), ["The target does not have slot container.26"]);
    assert_eq!(run(&mut server, "item modify block 1 -60 0 container.0 foo:bar"), ["Can't find element 'foo:bar' in registry 'minecraft:item_modifier'", "....0 foo:bar<--[HERE]"]);
    assert_eq!(
        run(&mut server, "item modify block 1 -60 0 container.0 {function:\"nope\"}"),
        [
            "Failed to parse structure: Failed to parse either. First: No key type in MapLike[{function:\"nope\"}]; Second: Failed to parse either. First: Not a string; Second: Failed to parse either. First: Not a list: {function:\"nope\"}; Second: No key type in MapLike[{function:\"nope\"}]",
            "...on:\"nope\"}<--[HERE]"
        ]
    );
    run(&mut server, "item replace block 7 -60 0 container.0 with shulker_box");
    let contents = "{type:\"minecraft:contents\",slot_source:{type:\"minecraft:slot_range\",slots:\"container.0\"},component:\"minecraft:container\"}";
    assert_eq!(run(&mut server, &format!("item fill block 7 -60 0 {contents} with stone 3")), ["Replaced 256 slot(s) at 7, -60, 0 with [Stone]"]);
    assert_eq!(run(&mut server, &format!("item override block 7 -60 0 {{type:\"minecraft:limit_slots\",limit:3,slot_source:{contents}}} with apple")), ["Replaced 3 slot(s) at 7, -60, 0 with [Apple]"]);
    assert_eq!(run(&mut server, &format!("item modify block 7 -60 0 {contents} {{type:\"minecraft:set_count\",count:10}}")), ["Modified 254 slot(s) at 7, -60, 0"]);
    assert_eq!(run(&mut server, &format!("execute if slots block 7 -60 0 {contents}")), ["Test passed. Count: 256"]);
    assert_eq!(run(&mut server, &format!("execute if items block 7 -60 0 {contents} *")), ["Test passed. Count: 2540"]);
    run(&mut server, "item replace block 7 -60 0 container.1 with bundle");
    let bundle = "{type:\"minecraft:contents\",slot_source:{type:\"minecraft:slot_range\",slots:\"container.1\"},component:\"minecraft:bundle_contents\"}";
    assert_eq!(run(&mut server, &format!("item fill block 7 -60 0 {bundle} with stone 20")), ["Replaced 3 slot(s) at 7, -60, 0 with [Stone]"]);
    assert_eq!(run(&mut server, "data get block 7 -60 0 Items[1].components.\"minecraft:bundle_contents\""), ["7, -60, 0 has the following block data: [{count: 60, id: \"minecraft:stone\"}]"]);
    run(&mut server, "item replace block 7 -60 0 container.2 with crossbow");
    let projectiles = "{type:\"minecraft:contents\",slot_source:{type:\"minecraft:slot_range\",slots:\"container.2\"},component:\"minecraft:charged_projectiles\"}";
    assert_eq!(run(&mut server, &format!("item fill block 7 -60 0 {projectiles} with arrow")), ["Replaced 1024 slot(s) at 7, -60, 0 with [Arrow]"]);
}

#[test]
fn execute_conditions_and_compute_match_vanilla() {
    let mut server = flat_server();
    run(&mut server, "setblock 0 -60 0 minecraft:chest");
    run(&mut server, "item replace block 0 -60 0 container.0 with stone 7");
    assert_eq!(run(&mut server, "execute if items block 0 -60 0 container.* *"), ["Test passed. Count: 7"]);
    assert_eq!(run(&mut server, "execute if items block 0 -60 0 container.* #minecraft:logs"), ["Test failed"]);
    assert_eq!(run(&mut server, "execute unless items block 0 -60 0 container.* stone"), ["Test failed. Count: 7"]);
    assert_eq!(run(&mut server, "execute if slots block 0 -60 0 container.*"), ["Test passed. Count: 27"]);
    assert_eq!(run(&mut server, "execute if items block 0 -61 0 container.* *"), ["Source position 0, -61, 0 is not a container"]);
    assert_eq!(run(&mut server, "execute if predicate {type:\"minecraft:random_chance\",chance:1}"), ["Test passed"]);
    assert_eq!(run(&mut server, "execute unless predicate {type:\"minecraft:random_chance\",chance:1}"), ["Test failed"]);
    assert_eq!(run(&mut server, "execute if predicate minecraft:foo"), ["Can't find element 'minecraft:foo' in registry 'minecraft:predicate'", "...ecraft:foo<--[HERE]"]);
    assert_eq!(run(&mut server, "compute default integer {type:\"add\",inputs:[2,3]}"), ["Number provider returned value 5"]);
    assert_eq!(run(&mut server, "compute default float {type:\"constant\",value:2.5}"), ["Number provider returned value 2.5 (rounded to 2)"]);
    assert_eq!(run(&mut server, "compute default float {type:\"constant\",value:2.5} 2"), ["Number provider returned value 2.5 (rounded to 5)"]);
    assert_eq!(run(&mut server, "compute default float {type:\"constant\",value:-2.5}"), ["Number provider returned value -2.5 (rounded to -3)"]);
    assert_eq!(run(&mut server, "compute default integer {type:\"pow\",base:2,exponent:-1}"), ["Number provider returned invalid value (negative exponent)"]);
    assert_eq!(run(&mut server, "compute default integer {type:\"abs\",input:-2147483648}"), ["Number provider returned invalid value (Overflow to represent absolute value of Integer.MIN_VALUE)"]);
    assert_eq!(run(&mut server, "compute default integer {type:\"mul\",inputs:[65536,65536]}"), ["Number provider returned invalid value (Value 4294967296 can't be safely converted to int)"]);
    assert_eq!(run(&mut server, "compute default float {type:\"sqrt\",input:-1}"), ["Number provider returned invalid value (NaN)"]);
    assert_eq!(run(&mut server, "compute default integer {type:\"from_float\",input:1e20}"), ["Number provider returned invalid value (Value 9223372036854775807 can't be safely converted to int)"]);
    assert_eq!(run(&mut server, "compute default integer {}"), ["Failed to parse structure: Failed to parse either. First: Not a number; Second: No key type in MapLike[{}]", "...integer {}<--[HERE]"]);
    assert_eq!(run(&mut server, "compute block 0 -60 0 integer {type:\"conditional\",condition:{type:\"minecraft:match_block\",block:\"minecraft:chest\"},on_true:1,on_false:2}"), ["Number provider returned value 1"]);
    run(&mut server, "data modify storage t:s v set compute default float {type:\"constant\",value:1.5}");
    run(&mut server, "data modify storage t:s w set compute default integer {type:\"div\",left:1,right:0}");
    assert_eq!(run(&mut server, "data get storage t:s"), ["Storage t:s has the following contents: {v: 1.5f, w: 0}"]);
}

#[test]
fn loot_arguments_suggest_registry_elements() {
    let server = flat_server();
    let source = CommandSourceStack::server(&server);
    let suggestions = completion_suggestions(&server, &source, "/loot spawn 0 0 0 loot minecraft:chests/simple_d").unwrap();
    assert_eq!(suggestions.list.into_iter().map(|s| s.text).collect::<Vec<_>>(), ["minecraft:chests/simple_dungeon"]);
    let suggestions = completion_suggestions(&server, &source, "/item replace block 0 0 0 weapon.").unwrap();
    assert_eq!(suggestions.list.into_iter().map(|s| s.text).collect::<Vec<_>>(), ["weapon.*", "weapon.mainhand", "weapon.offhand"]);
}

#[test]
fn loot_functions_resolve_entity_text_like_vanilla() {
    let mut server = flat_server();
    run(&mut server, "setblock 0 -60 0 minecraft:chest");
    run(&mut server, "scoreboard objectives add o dummy");
    run(&mut server, "summon pig 2 -60 2 {NoAI:1b,Tags:[\"u\"]}");
    run(&mut server, "summon pig 4 -60 2 {NoAI:1b,Tags:[\"u\"]}");
    run(&mut server, "scoreboard players set @e[tag=u] o 9");
    let table = |slot: u32, function: &str| {
        format!("execute as @e[tag=u,limit=1] run loot replace block 0 -60 0 container.{slot} loot {{pools:[{{rolls:1,entries:[{{type:\"minecraft:item\",name:\"minecraft:stone\",modifier:{function}}}]}}]}}")
    };
    run(&mut server, &table(0, "{type:\"minecraft:set_name\",name:{score:{name:\"@s\",objective:\"o\"}},entity:\"this\"}"));
    run(&mut server, &table(1, "{type:\"minecraft:set_name\",name:{score:{name:\"@s\",objective:\"o\"}}}"));
    run(&mut server, &table(2, "{type:\"minecraft:set_name\",name:{score:{name:\"@e[tag=u]\",objective:\"o\"}},entity:\"this\"}"));
    run(&mut server, &table(3, "{type:\"minecraft:set_lore\",lore:[{score:{name:\"@s\",objective:\"o\"}},\"plain\"],mode:\"append\",entity:\"this\"}"));
    assert_eq!(run(&mut server, "data get block 0 -60 0 Items[{Slot:0b}].components"), ["0, -60, 0 has the following block data: {\"minecraft:custom_name\": \"9\"}"]);
    assert_eq!(
        run(&mut server, "data get block 0 -60 0 Items[{Slot:1b}].components"),
        ["0, -60, 0 has the following block data: {\"minecraft:custom_name\": {score: {name: \"@s\", objective: \"o\"}}}"]
    );
    assert_eq!(
        run(&mut server, "data get block 0 -60 0 Items[{Slot:2b}].components"),
        ["0, -60, 0 has the following block data: {\"minecraft:custom_name\": {score: {name: \"@e[tag=u]\", objective: \"o\"}}}"]
    );
    assert_eq!(run(&mut server, "data get block 0 -60 0 Items[{Slot:3b}].components"), ["0, -60, 0 has the following block data: {\"minecraft:lore\": [\"9\", \"plain\"]}"]);
}

#[test]
fn malformed_entity_predicates_print_vanilla_errors() {
    let mut server = flat_server();
    let predicate = |inner: &str| format!("execute if predicate {{type:\"minecraft:entity_properties\",entity:\"this\",predicate:{inner}}}");
    assert_eq!(
        run(&mut server, &predicate("{flags:{is_baby:\"x\"}}")),
        ["Failed to parse structure: Not a number missed input: {flags:{is_baby:\"x\"}}", "...aby:\"x\"}}}<--[HERE]"]
    );
    assert_eq!(
        run(&mut server, &predicate("{bogus:1,flags:{is_baby:\"x\"}}")),
        [
            "Failed to parse structure: Not a number; Unknown registry key in ResourceKey[minecraft:root / minecraft:entity_sub_predicate_type]: minecraft:bogus; Unknown registry key in ResourceKey[minecraft:root / minecraft:entity_sub_predicate_type]: minecraft:bogus missed input: {bogus:1,flags:{is_baby:\"x\"}}",
            "...aby:\"x\"}}}<--[HERE]"
        ]
    );
    assert_eq!(
        run(&mut server, &predicate("{\"minecraft:type_specific/player\":{looking_at:{team:5}}}")),
        [
            "Failed to parse structure: Not a string missed input: {team:5} missed input: {\"minecraft:type_specific/player\":{looking_at:{team:5}}}",
            "...team:5}}}}<--[HERE]"
        ]
    );
}

#[test]
fn malformed_loot_functions_print_vanilla_errors() {
    let mut server = flat_server();
    run(&mut server, "setblock 0 -60 0 minecraft:chest");
    let name_error = "Failed to parse structure: Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 5; Second: Not a map: 5";
    let lore_error = "Failed to parse structure: Failed to parse either. First: No key mode in MapLike[{lore:5,type:\"minecraft:set_lore\"}]; Not a list: 5; Second: Failed to parse either. First: Not a string; Second: Failed to parse either. First: Not a list: {lore:5,type:\"minecraft:set_lore\"}; Second: No key mode in MapLike[{lore:5,type:\"minecraft:set_lore\"}]; Not a list: 5";
    let table = |function: &str| format!("loot spawn 0 -60 0 loot {{pools:[{{rolls:1,entries:[{{type:\"minecraft:item\",name:\"minecraft:stone\",modifier:{function}}}]}}]}}");
    assert_eq!(run(&mut server, &table("{type:\"minecraft:set_name\",name:5}")), [name_error, "...me:5}}]}]}<--[HERE]"]);
    assert_eq!(run(&mut server, &table("{type:\"minecraft:set_lore\",lore:5}")), [lore_error, "...re:5}}]}]}<--[HERE]"]);
    assert_eq!(
        run(&mut server, &table("{function:\"minecraft:set_name\",name:5}")),
        [
            "Failed to parse structure: Failed to parse either. First: No key type in MapLike[{function:\"minecraft:set_name\",name:5}]; Second: Failed to parse either. First: Not a string; Second: Failed to parse either. First: Not a list: {function:\"minecraft:set_name\",name:5}; Second: No key type in MapLike[{function:\"minecraft:set_name\",name:5}]",
            "...me:5}}]}]}<--[HERE]"
        ]
    );
    let modify = |function: &str| format!("item modify block 0 -60 0 container.0 {function}");
    assert_eq!(run(&mut server, &modify("{type:\"minecraft:set_lore\",lore:5}")), [lore_error, "...e\",lore:5}<--[HERE]"]);
    assert_eq!(run(&mut server, &modify("{type:\"minecraft:set_name\",name:5}")), [name_error, "...e\",name:5}<--[HERE]"]);
    assert_eq!(
        run(&mut server, &modify("[{type:\"minecraft:set_name\",name:5},{type:\"minecraft:set_count\"}]")),
        [
            "Failed to parse structure: No key count in MapLike[{type:\"minecraft:set_count\"}]; Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 5; Second: Not a map: 5",
            "...t_count\"}]<--[HERE]"
        ]
    );
    assert_eq!(
        run(&mut server, &modify("{type:\"minecraft:set_name\",name:5,entity:\"x\",target:\"y\",condition:5}")),
        [
            "Failed to parse structure: Unknown element name:y; Unknown element name:x; Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 5; Second: Not a map: 5; Not a map: 5",
            "...ndition:5}<--[HERE]"
        ]
    );
    assert_eq!(
        run(&mut server, &modify("{type:\"minecraft:set_lore\",mode:\"append\",lore:[\"a\",{text:1}],condition:5}")),
        [
            "Failed to parse structure: Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found; Not a map: 5",
            "...ndition:5}<--[HERE]"
        ]
    );
}
