use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::util::datafix::schemas::{v1451_6, v3818_5, v704, v705};

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

#[test]
#[ignore]
fn updates_match_the_vanilla_oracle() {
    let cases = std::fs::read_to_string(std::env::var("WAFER_DFU_CASES").expect("WAFER_DFU_CASES points at the DfuOracle input")).unwrap();
    let expected = std::fs::read_to_string(std::env::var("WAFER_DFU_EXPECTED").expect("WAFER_DFU_EXPECTED points at the DfuOracle output")).unwrap();
    let mut expected = expected.lines();
    let mut mismatches = 0;
    for case in cases.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = case.splitn(4, '\t').collect();
        let ty: &'static str = Box::leak(fields[0].to_string().into_boxed_str());
        let actual = update(ty, fields[1].parse().unwrap(), fields[2].parse().unwrap(), fields[3]);
        let vanilla = expected.next().expect("an expected line");
        if actual != vanilla {
            mismatches += 1;
            eprintln!("{case}\n  vanilla: {vanilla}\n  wafer:   {actual}");
        }
    }
    assert_eq!(mismatches, 0);
}

#[test]
fn schema_hooks_match_vanilla() {
    let cases: [(&str, &str, &str); 44] = [
        ("unpack", r#"{CriteriaName:"minecraft.mined:minecraft.stone",DisplayName:"x"}"#, r#"{CriteriaName:"minecraft.mined:minecraft.stone",CriteriaType:{id:"minecraft:stone",type:"minecraft:mined"},DisplayName:"x"}"#),
        ("unpack", r#"{CriteriaName:"dummy"}"#, r#"{CriteriaName:"dummy",CriteriaType:{id:"dummy",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"a:b:c"}"#, r#"{CriteriaName:"a:b:c",CriteriaType:{id:"a:b:c",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"minecraft.custom:minecraft.jump"}"#, r#"{CriteriaName:"minecraft.custom:minecraft.jump",CriteriaType:{id:"minecraft:jump",type:"minecraft:custom"}}"#),
        ("unpack", r#"{CriteriaName:"Bad Space:x"}"#, r#"{CriteriaName:"Bad Space:x",CriteriaType:{id:"Bad Space:x",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"stat.mineBlock.minecraft.stone"}"#, r#"{CriteriaName:"stat.mineBlock.minecraft.stone",CriteriaType:{id:"stat.mineBlock.minecraft.stone",type:"_special"}}"#),
        ("unpack", r#"{CriteriaName:"x:"}"#, r#"{CriteriaName:"x:",CriteriaType:{id:"minecraft:",type:"minecraft:x"}}"#),
        ("unpack", r#"{CriteriaName:":y"}"#, r#"{CriteriaName:":y",CriteriaType:{id:"minecraft:y",type:"minecraft:"}}"#),
        ("unpack", r#"{DisplayName:"x"}"#, r#"{DisplayName:"x"}"#),
        ("unpack", r#"{CriteriaName:5}"#, r#"{CriteriaName:5}"#),
        ("repack", r#"{CriteriaType:{type:"minecraft:mined",id:"minecraft:stone"},DisplayName:"x"}"#, r#"{CriteriaName:"minecraft.mined:minecraft.stone",DisplayName:"x"}"#),
        ("repack", r#"{CriteriaType:{type:"_special",id:"dummy"}}"#, r#"{CriteriaName:"dummy"}"#),
        ("repack", r#"{CriteriaType:{type:"a"}}"#, r#"{CriteriaType:{type:"a"}}"#),
        ("repack", r#"{CriteriaType:{type:"minecraft:custom",id:"minecraft:jump"}}"#, r#"{CriteriaName:"minecraft.custom:minecraft.jump"}"#),
        ("repack", r#"{CriteriaType:{type:"Bad Space",id:"x"}}"#, r#"{CriteriaName:"Bad Space:minecraft.x"}"#),
        ("repack", r#"{CriteriaType:5}"#, r#"{CriteriaType:5}"#),
        ("repack", r#"{DisplayName:"x"}"#, r#"{DisplayName:"x"}"#),
        ("plain", r#""minecraft:stone""#, r#"{count:1,id:"minecraft:stone"}"#),
        ("plain", r#"{id:"x"}"#, r#"{id:"x"}"#),
        ("plain", r#"5"#, r#"5"#),
        ("v704", r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[]}}}"#, r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[],id:"minecraft:chest"}}}"#),
        ("v704", r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{id:"minecraft:sign"}}}"#),
        ("v704", r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{x:1}}}"#, r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{id:"minecraft:shulker_box",x:1}}}"#),
        ("v704", r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{}}}"#),
        ("v704", r#"{id:"minecraft:armor_stand",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:armor_stand",tag:{EntityTag:{id:"ArmorStand"}}}"#),
        ("v704", r#"{id:"minecraft:boat",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:boat",tag:{EntityTag:{}}}"#),
        ("v704", r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#, r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#),
        ("v704", r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#),
        ("v704", r#"{tag:{BlockEntityTag:{}}}"#, r#"{tag:{BlockEntityTag:{}}}"#),
        ("v704", r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{id:"minecraft:enchanting_table"}}}"#),
        ("v704", r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{id:"minecraft:mob_spawner"}}}"#),
        ("v704", r#"{id:"minecraft:stone"}"#, r#"{id:"minecraft:stone"}"#),
        ("v705", r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[]}}}"#, r#"{id:"minecraft:chest",tag:{BlockEntityTag:{Items:[],id:"minecraft:chest"}}}"#),
        ("v705", r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:oak_sign",tag:{BlockEntityTag:{id:"minecraft:sign"}}}"#),
        ("v705", r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{x:1}}}"#, r#"{id:"minecraft:white_shulker_box",tag:{BlockEntityTag:{id:"minecraft:shulker_box",x:1}}}"#),
        ("v705", r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:pig_spawn_egg",tag:{EntityTag:{id:"minecraft:pig"}}}"#),
        ("v705", r#"{id:"minecraft:armor_stand",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:armor_stand",tag:{EntityTag:{id:"minecraft:armor_stand"}}}"#),
        ("v705", r#"{id:"minecraft:boat",tag:{EntityTag:{}}}"#, r#"{id:"minecraft:boat",tag:{EntityTag:{id:"minecraft:oak_boat"}}}"#),
        ("v705", r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#, r#"{id:"minecraft:painting",tag:{EntityTag:{id:"keep"}}}"#),
        ("v705", r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:stone",tag:{BlockEntityTag:{}}}"#),
        ("v705", r#"{tag:{BlockEntityTag:{}}}"#, r#"{tag:{BlockEntityTag:{}}}"#),
        ("v705", r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:enhanting_table",tag:{BlockEntityTag:{id:"minecraft:enchanting_table"}}}"#),
        ("v705", r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{}}}"#, r#"{id:"minecraft:spawner",tag:{BlockEntityTag:{id:"minecraft:mob_spawner"}}}"#),
        ("v705", r#"{id:"minecraft:stone"}"#, r#"{id:"minecraft:stone"}"#),
    ];
    for (label, input, expected) in cases {
        let hook: fn(Dynamic) -> Dynamic = match label {
            "unpack" => v1451_6::unpack_objective_id,
            "repack" => v1451_6::repack_objective_id,
            "plain" => v3818_5::unpack_plain_id,
            "v704" => v704::add_names_hook,
            "v705" => v705::add_names_hook,
            _ => unreachable!(),
        };
        let value = crate::nbt::parse_fully(input).expect("valid snbt");
        let output = hook(Dynamic::new(value));
        assert_eq!(crate::nbt::to_string(output.get_value().expect("a value")), expected, "{label} {input}");
    }
}
