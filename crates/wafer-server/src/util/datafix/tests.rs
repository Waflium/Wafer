use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;

fn update(ty: &'static str, from: i32, to: i32, input: &str) -> String {
    let input = crate::nbt::parse_fully(input).expect("valid snbt");
    let output = get_data_fixer().update(TypeReference(ty), Dynamic::new(input), from, to);
    crate::nbt::to_string(output.get_value().expect("a value"))
}

#[test]
fn equipment_splits_into_armor_and_hand_items() {
    assert_eq!(
        update("entity_chunk", 99, 100, r#"{Entities:[{id:"Zombie",Equipment:[{id:"stone",Count:1b},{id:"minecraft:iron_boots",Count:1b}],DropChances:[0.5f,0.25f,0.1f]},{id:"Pig"},{id:"Skeleton",Equipment:[{id:"bow"}]}]}"#),
        r#"{Entities:[{ArmorDropChances:[0.25f,0.1f,0.0f,0.0f],ArmorItems:[{Count:1b,id:"minecraft:iron_boots"},{},{},{}],Equipment:[{Count:1b,id:"stone"},{Count:1b,id:"minecraft:iron_boots"}],HandDropChances:[0.5f,0.0f],HandItems:[{Count:1b,id:"minecraft:stone"},{}],id:"Zombie"},{id:"Pig"},{Equipment:[{id:"bow"}],HandItems:[{id:"minecraft:bow"},{}],id:"Skeleton"}]}"#
    );
    assert_eq!(update("player", 99, 100, r#"{Inventory:[{id:"stone",Count:2b}]}"#), r#"{Inventory:[{Count:2b,id:"minecraft:stone"}]}"#);
    assert_eq!(
        update("entity_chunk", 99, 100, r#"{Entities:[{id:"Item",Item:{id:"chest",tag:{BlockEntityTag:{Items:[{id:"dirt"}]}}}}]}"#),
        r#"{Entities:[{Item:{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[{id:"minecraft:dirt"}],id:"Chest"}}},id:"Item"}]}"#
    );
}

#[test]
#[ignore]
fn schemas_match_the_vanilla_dump() {
    let path = std::env::var("WAFER_SCHEMA_DUMP").expect("WAFER_SCHEMA_DUMP points at the SchemaDumpOracle output");
    let expected = std::fs::read_to_string(path).unwrap();
    let max: i32 = std::env::var("WAFER_SCHEMA_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(i32::MAX);
    let fixer = get_data_fixer();
    let mut mismatches = 0;
    for line in expected.lines() {
        let (key, rest) = line.split_once(' ').unwrap();
        let (name, canonical) = rest.split_once(" = ").unwrap();
        let key: i32 = key.parse().unwrap();
        if key > max {
            break;
        }
        let schema = fixer.get_schema(key);
        assert_eq!(schema.version_key, key, "schema {key} is missing");
        let actual = crate::datafixers::types::canonical(&schema.get_type(TypeReference(Box::leak(name.to_string().into_boxed_str()))));
        if actual != canonical {
            mismatches += 1;
            if mismatches <= 5 {
                eprintln!("{key} {name}\n  vanilla: {canonical}\n  wafer:   {actual}");
            }
        }
    }
    assert_eq!(mismatches, 0);
}
