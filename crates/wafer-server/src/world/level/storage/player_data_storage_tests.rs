use wafer_protocol::nbt::Nbt;

use super::player_data_storage::PlayerDataStorage;
use super::{WorldDir, format_uuid, read_compressed, read_saved_data, read_tag_from_disk, write_compressed};
use crate::util::codec::DecodeResult;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::level::chunk::storage::encode_nbt;

const UUID: u128 = 0xd76279edcc573f1885f4367c1defa1ee;

fn world(name: &str) -> WorldDir {
    let root = std::env::temp_dir().join(format!("wafer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("players/data")).unwrap();
    std::fs::create_dir_all(root.join("data/minecraft")).unwrap();
    WorldDir { root }
}

fn player_tag(level: i32) -> Nbt {
    Nbt::Compound(vec![("DataVersion".into(), Nbt::Int(wafer_data::WORLD_VERSION)), ("XpLevel".into(), Nbt::Int(level))].into())
}

fn entity_chunk_with_broken_uuid() -> Nbt {
    let pig = Nbt::Compound(vec![("id".into(), Nbt::String("Pig".into())), ("UUID".into(), Nbt::String("not a uuid".into()))].into());
    Nbt::Compound(vec![("DataVersion".into(), Nbt::Int(100)), ("Entities".into(), Nbt::List(vec![pig])), ("Position".into(), Nbt::IntArray(vec![0, 0]))].into())
}

fn player_files(dir: &WorldDir) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.root.join("players/data")).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn saving_keeps_the_previous_file_as_dat_old() {
    let dir = world("player-data-save");
    let storage = PlayerDataStorage::new(&dir);
    storage.save(&player_tag(1), UUID, "Steve");
    assert_eq!(player_files(&dir), vec![format!("{}.dat", format_uuid(UUID))]);
    storage.save(&player_tag(2), UUID, "Steve");
    assert_eq!(player_files(&dir), vec![format!("{}.dat", format_uuid(UUID)), format!("{}.dat_old", format_uuid(UUID))]);
    let old = read_compressed(&dir.root.join(format!("players/data/{}.dat_old", format_uuid(UUID)))).unwrap();
    assert_eq!(old.get("XpLevel"), Some(&Nbt::Int(1)));
    assert_eq!(storage.load("Steve", UUID).unwrap().get("XpLevel"), Some(&Nbt::Int(2)));
    std::fs::remove_dir_all(dir.root).unwrap();
}

#[test]
fn a_corrupted_file_is_backed_up_and_dat_old_is_loaded() {
    let dir = world("player-data-corrupted");
    let storage = PlayerDataStorage::new(&dir);
    storage.save(&player_tag(1), UUID, "Steve");
    storage.save(&player_tag(2), UUID, "Steve");
    let real = dir.root.join(format!("players/data/{}.dat", format_uuid(UUID)));
    std::fs::write(&real, b"not gzip").unwrap();
    assert_eq!(storage.load("Steve", UUID).unwrap().get("XpLevel"), Some(&Nbt::Int(1)));
    let files = player_files(&dir);
    let backup = files.iter().find(|name| name.starts_with(&format!("{}_corrupted_", format_uuid(UUID)))).expect("corrupted backup");
    assert!(backup.ends_with(".dat"));
    assert_eq!(std::fs::read(dir.root.join("players/data").join(backup)).unwrap(), b"not gzip");
    std::fs::remove_dir_all(dir.root).unwrap();
}

#[test]
fn a_missing_file_falls_back_to_dat_old_without_a_backup() {
    let dir = world("player-data-missing");
    write_compressed(&player_tag(5), &dir.root.join(format!("players/data/{}.dat_old", format_uuid(UUID)))).unwrap();
    assert_eq!(PlayerDataStorage::new(&dir).load("Steve", UUID).unwrap().get("XpLevel"), Some(&Nbt::Int(5)));
    assert_eq!(player_files(&dir), vec![format!("{}.dat_old", format_uuid(UUID))]);
    std::fs::remove_dir_all(dir.root).unwrap();
}

#[test]
fn saved_data_may_be_uncompressed() {
    let dir = world("saved-data-uncompressed");
    let data = Nbt::Compound(vec![("DataVersion".into(), Nbt::Int(wafer_data::WORLD_VERSION)), ("data".into(), Nbt::Compound(vec![("value".into(), Nbt::Int(3))].into()))].into());
    let file = dir.data("weather");
    std::fs::write(&file, encode_nbt(&data)).unwrap();
    let read = read_saved_data(&file, "main", "weather", DataFixTypes::SavedDataWeather, |data| DecodeResult::success(data.clone()));
    assert_eq!(read, Some(Nbt::Compound(vec![("value".into(), Nbt::Int(3))].into())));
    std::fs::remove_dir_all(dir.root).unwrap();
}

#[test]
fn a_data_fixer_exception_is_an_error_instead_of_a_panic() {
    let dir = world("saved-data-fixer-exception");
    let file = dir.root.join("entities.dat");
    write_compressed(&entity_chunk_with_broken_uuid(), &file).unwrap();
    let error = read_tag_from_disk(&file, DataFixTypes::EntityChunk, wafer_data::WORLD_VERSION).unwrap_err();
    assert!(error.to_string().contains("Invalid UUID string: not a uuid"), "{error}");
    std::fs::remove_dir_all(dir.root).unwrap();
}
