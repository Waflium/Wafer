use wafer_data::registry;

use super::map_decoration_types::TYPES;
use super::map_index::MapIndex;
use super::map_item_saved_data::MapItemSavedData;

#[test]
fn decoration_types_follow_the_registry() {
    assert_eq!(registry::MAP_DECORATION_TYPE.len(), TYPES.len());
    for (key, ty) in registry::MAP_DECORATION_TYPE.iter().zip(TYPES.iter()) {
        assert_eq!(*key, format!("minecraft:{}", ty.name));
    }
}

#[test]
fn fresh_maps_are_centered_like_vanilla() {
    let data = MapItemSavedData::create_fresh(0.0, 0.0, 0, true, false, "minecraft:overworld");
    assert_eq!((data.center_x, data.center_z), (0, 0));
    let data = MapItemSavedData::create_fresh(100.0, -100.0, 0, true, false, "minecraft:overworld");
    assert_eq!((data.center_x, data.center_z), (128, -128));
    let data = MapItemSavedData::create_fresh(100.0, -100.0, 2, true, false, "minecraft:overworld");
    assert_eq!((data.center_x, data.center_z), (192, -320));
}

#[test]
fn saved_data_round_trips() {
    let mut data = MapItemSavedData::create_fresh(5.0, 7.0, 1, false, true, "minecraft:the_nether");
    data.colors[129] = 42;
    let encoded = data.encode();
    assert_eq!(encoded.get("scale"), Some(&wafer_protocol::nbt::Nbt::Byte(1)));
    assert!(encoded.get("locked").is_none());
    let decoded = MapItemSavedData::decode(&encoded).value.unwrap();
    assert_eq!(decoded.encode(), encoded);
    let mut index = MapIndex::default();
    assert_eq!(index.encode(), wafer_protocol::nbt::Nbt::Compound(Vec::new()));
    assert_eq!(index.get_next_map_id().id(), 0);
    assert_eq!(MapIndex::decode(&index.encode()).value.unwrap().get_next_map_id().id(), 1);
}

#[test]
fn holders_in_another_dimension_are_dropped_when_they_lose_the_map() {
    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::network::login::Profile;
    use crate::server::level::testing;
    use wafer_core::Vec3;
    let mut server = testing::server();
    let a = testing::server_player_with_profile(&mut server, Profile { uuid: 1, name: "A".into(), properties: Vec::new() }, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let b = testing::server_player_with_profile(&mut server, Profile { uuid: 2, name: "B".into(), properties: Vec::new() }, Vec3::new(2.5, -60.0, 0.5), 0.0, 0.0);
    let map = crate::world::item::map_item::create(server.overworld_mut(), 0, 0, 0, true, false);
    let id = crate::world::level::saveddata::maps::map_id::MapId::from_nbt(map.get(crate::world::item::component::MAP_ID).unwrap()).unwrap();
    server.overworld_mut().player_mut(a).inventory.slots[crate::world::item::inventory_slot(0)] = Some(map.clone());
    server.overworld_mut().player_mut(b).inventory.slots[crate::world::item::inventory_slot(0)] = Some(map);
    server.overworld_mut().handle_player_loaded(a);
    server.overworld_mut().handle_player_loaded(b);
    for _ in 0..3 {
        server.tick();
    }
    let data = server.overworld().get_map_data(id).unwrap();
    assert!(data.borrow().is_carried_by(b));
    let console = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &console, "execute in minecraft:the_nether run tp B 0 100 0");
    perform_prefixed_command(&mut server, &console, "clear B");
    for _ in 0..3 {
        server.tick();
    }
    assert!(data.borrow().is_carried_by(a));
    assert!(!data.borrow().is_carried_by(b));
}

#[test]
fn saved_data_decodes_partially_with_vanilla_messages() {
    let decode = |snbt: &str| {
        let result = crate::util::codec::with_nbt_ops(|| MapItemSavedData::decode(&crate::world::item::component::custom::parse_snbt(snbt).unwrap()));
        (result.error.unwrap_or_else(|| "-".to_string()), result.value.map(|data| (data.encode().get("banners").map_or(0, |banners| banners.as_list().len()), data.scale, data.locked)))
    };
    assert_eq!(
        decode(r#"{dimension:"minecraft:overworld",xCenter:1,zCenter:2,colors:[B;],banners:[{pos:[I;1,2,3]},{pos:[I;1,2]},{pos:[I;4,5,6],color:"nope",name:{bad:1}}],scale:"x",locked:"y"}"#),
        ("Input is not a list of 3 ints; Not a number; Not a number".to_string(), Some((2, 0, false)))
    );
    assert_eq!(
        decode(r#"{dimension:"Bad Id",xCenter:"a",colors:[1,"b"]}"#),
        (
            r#"Some elements are not bytes: [1,"b"]; No key zCenter in MapLike[{colors:[1,"b"],dimension:"Bad Id",xCenter:"a"}]; Not a number; Not a valid resource location: Bad Id Non [a-z0-9/._-] character in path of location: minecraft:Bad Id"#.to_string(),
            None
        )
    );
    assert_eq!(
        decode(r#"{dimension:"overworld",xCenter:1,zCenter:2,colors:[1,2],frames:[{pos:[I;0,0,0],rotation:1}]}"#),
        ("No key entity_id in MapLike[{pos:[I;0,0,0],rotation:1}]".to_string(), Some((0, 0, false)))
    );
}
