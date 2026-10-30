use wafer_core::java_string::JavaStr;
use wafer_protocol::nbt::{Nbt, TagMap};

use crate::commands::arguments::nbt_path_argument::{NbtPath, parse};
use crate::commands::brigadier::StringReader;
use crate::world::item::component as dc;
use crate::world::level::spawn_data::SpawnData;
use crate::world::level::storage::command_storage::CommandStorage;

fn grown(keep: &[&str]) -> TagMap {
    let mut fields = TagMap::new();
    for i in 0..13 {
        fields.put(&format!("grow{i}"), Nbt::Int(i));
    }
    for key in keep {
        fields.put(key, Nbt::Int(key.len() as i32));
    }
    for i in 0..13 {
        fields.remove_key(&format!("grow{i}"));
    }
    fields
}

fn keys(fields: &TagMap) -> Vec<&str> {
    fields.iteration().map(|(key, _)| key.as_str()).collect()
}

fn compound_keys(tag: &Nbt) -> Vec<&str> {
    let Nbt::Compound(fields) = tag else { panic!("not a compound: {tag:?}") };
    keys(fields)
}

fn path(text: &str) -> NbtPath {
    let units: Vec<u16> = text.java_utf16().collect();
    parse(&mut StringReader::new(&units)).unwrap()
}

#[test]
fn nbt_path_set_stores_a_copy() {
    let value = Nbt::Compound(grown(&["ax", "h", "i"]));
    assert_eq!(compound_keys(&value), ["h", "i", "ax"]);
    let mut root = Nbt::Compound(TagMap::new());
    assert_eq!(path("a.b").set(&mut root, &value).unwrap(), 1);
    assert_eq!(compound_keys(root.get("a").unwrap().get("b").unwrap()), ["ax", "h", "i"]);
}

#[test]
fn nbt_path_insert_stores_a_copy() {
    let mut root = Nbt::Compound(vec![("list".to_string(), Nbt::List(Vec::new()))].into());
    path("list").insert(-1, &mut root, &[Nbt::Compound(grown(&["ax", "h", "i"]))]).unwrap();
    assert_eq!(compound_keys(&root.get("list").unwrap().as_list()[0]), ["ax", "h", "i"]);
}

#[test]
fn command_storage_keeps_the_capacity_until_a_copy_is_made() {
    let mut storage = CommandStorage::new(None);
    storage.set("t:s", grown(&["ax", "h", "i"]));
    assert_eq!(keys(&storage.get("t:s")), ["h", "i", "ax"]);
    let fetched = storage.get("t:s");
    storage.write_back("t:s", fetched);
    assert_eq!(keys(&storage.get("t:s")), ["h", "i", "ax"]);
    assert_eq!(keys(&storage.get("t:s").copy()), ["ax", "h", "i"]);
}

#[test]
fn command_storage_saves_and_loads_copies() {
    let dir = std::env::temp_dir().join(format!("wafer-command-storage-copy-{}", std::process::id()));
    let mut storage = CommandStorage::new(Some(dir.clone()));
    storage.set("t:s", grown(&["ax", "h", "i"]));
    storage.save().unwrap();
    let mut loaded = CommandStorage::new(Some(dir.clone()));
    assert_eq!(keys(&loaded.get("t:s")), ["ax", "h", "i"]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn spawn_data_keeps_the_capacity_of_the_removed_id_and_encodes_a_copy() {
    let mut entity = TagMap::new();
    for i in 0..12 {
        entity.put(&format!("s{i}"), Nbt::Int(i));
    }
    entity.put("id", Nbt::String("not an identifier".into()));
    let spawn_data = SpawnData::new(entity, None, None);
    assert_eq!(compound_keys(spawn_data.get_entity_to_spawn()), ["s3", "s4", "s5", "s6", "s7", "s8", "s9", "s11", "s10", "s0", "s1", "s2"]);
    let encoded = spawn_data.encode();
    assert_eq!(compound_keys(encoded.get("entity").unwrap()), ["s3", "s4", "s5", "s11", "s6", "s10", "s7", "s8", "s9", "s0", "s1", "s2"]);
}

#[test]
fn typed_entity_data_keeps_the_capacity_of_the_stripped_id() {
    let mut source = TagMap::new();
    source.put("id", Nbt::String("minecraft:pig".into()));
    for i in 0..12 {
        source.put(&format!("t{i}"), Nbt::Int(i));
    }
    assert_eq!(keys(&source), ["t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11", "id", "t0", "t1", "t2", "t3"]);
    let decoded = dc::ENTITY_DATA.decode(&Nbt::Compound(source)).unwrap();
    let (id, unsafe_data) = dc::values::typed_entity_data(&decoded);
    assert_eq!(id, "minecraft:pig");
    assert_eq!(compound_keys(&unsafe_data), ["t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11", "t0", "t1", "t2", "t3"]);
    assert_eq!(compound_keys(&unsafe_data.copy()), ["t4", "t5", "t6", "t10", "t7", "t8", "t9", "t11", "t0", "t1", "t2", "t3"]);
    let encoded = dc::ENTITY_DATA.encode(&decoded);
    assert_eq!(compound_keys(&encoded), ["t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11", "id", "t0", "t1", "t2", "t3"]);
}

#[test]
fn custom_data_components_encode_copies() {
    let value = Nbt::Compound(grown(&["ax", "h", "i"]));
    assert_eq!(compound_keys(&dc::CUSTOM_DATA.encode(&value)), ["ax", "h", "i"]);
    assert_eq!(compound_keys(&dc::BUCKET_ENTITY_DATA.encode(&value)), ["ax", "h", "i"]);
    assert_eq!(compound_keys(&dc::LORE.encode(&value)), ["h", "i", "ax"]);
}

#[test]
fn custom_data_components_decode_copies() {
    let value = Nbt::Compound(grown(&["ax", "h", "i"]));
    assert_eq!(compound_keys(&dc::CUSTOM_DATA.decode(&value).unwrap()), ["ax", "h", "i"]);
}
