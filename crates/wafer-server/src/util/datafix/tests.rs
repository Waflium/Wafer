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
