use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use crate::server::notifications::NotificationManager;
use crate::world::level::chunk::storage::{RegionFile, decode_compressed_nbt};
use crate::world::level::storage::level_storage_source::LevelStorageSource;

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn canonical(tag: &Nbt, out: &mut String) {
    match tag {
        Nbt::End => out.push_str("END"),
        Nbt::Compound(entries) => {
            let mut sorted: Vec<&(String, Nbt)> = entries.iter().collect();
            sorted.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (index, (key, value)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&quote(key));
                out.push(':');
                canonical(value, out);
            }
            out.push('}');
        }
        Nbt::List(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonical(value, out);
            }
            out.push(']');
        }
        Nbt::String(value) => out.push_str(&quote(value)),
        Nbt::End => out.push_str("END"),
        Nbt::Byte(value) => out.push_str(&format!("{value}b")),
        Nbt::Short(value) => out.push_str(&format!("{value}s")),
        Nbt::Int(value) => out.push_str(&value.to_string()),
        Nbt::Long(value) => out.push_str(&format!("{value}L")),
        Nbt::Float(value) => out.push_str(&format!("{}f", crate::nbt::java_float_to_string(*value))),
        Nbt::Double(value) => out.push_str(&format!("{}d", crate::nbt::java_double_to_string(*value))),
        Nbt::LazilyParsedNumber(text) => out.push_str(text),
        Nbt::ByteArray(values) => {
            out.push_str("[B;");
            out.push_str(&values.iter().map(|value| format!("{value}b")).collect::<Vec<_>>().join(","));
            out.push(']');
        }
        Nbt::IntArray(values) => {
            out.push_str("[I;");
            out.push_str(&values.iter().map(i32::to_string).collect::<Vec<_>>().join(","));
            out.push(']');
        }
        Nbt::LongArray(values) => {
            out.push_str("[L;");
            out.push_str(&values.iter().map(|value| format!("{value}L")).collect::<Vec<_>>().join(","));
            out.push(']');
        }
    }
}

fn canonical_string(tag: &Nbt) -> String {
    let mut out = String::new();
    canonical(tag, &mut out);
    out
}

fn collect(root: &Path, directory: &Path, entries: &mut Vec<(String, PathBuf, bool)>) {
    let mut children: Vec<PathBuf> = std::fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().path()).collect();
    children.sort();
    for child in children {
        let relative = child.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        let is_directory = child.is_dir();
        entries.push((relative, child.clone(), is_directory));
        if is_directory {
            collect(root, &child, entries);
        }
    }
}

pub fn dump(root: &Path) -> String {
    let mut entries = Vec::new();
    collect(root, root, &mut entries);
    entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let mut out = String::new();
    for (relative, path, is_directory) in entries {
        if is_directory {
            out.push_str(&format!("D {relative}\n"));
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with(".mca") {
            out.push_str(&format!("REGION {relative}\n"));
            let parts: Vec<&str> = name.split('.').collect();
            let region_x: i32 = parts[1].parse().unwrap();
            let region_z: i32 = parts[2].parse().unwrap();
            let mut region = RegionFile::open(&path, false).unwrap();
            for z in 0..32 {
                for x in 0..32 {
                    let pos = ChunkPos::new(region_x * 32 + x, region_z * 32 + z);
                    match region.read(pos) {
                        Ok(Some(tag)) => out.push_str(&format!("CHUNK {} {} {}\n", pos.x, pos.z, canonical_string(&tag))),
                        Ok(None) => {}
                        Err(_) => out.push_str(&format!("CHUNK {} {} ERROR\n", pos.x, pos.z)),
                    }
                }
            }
            continue;
        }
        if name.ends_with(".json") || name.ends_with(".txt") || name.ends_with(".properties") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.push_str(&format!("TEXT {relative} {}\n", text.replace('\r', "\\r").replace('\n', "\\n")));
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let gz = bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b;
        if gz && (name.ends_with(".dat") || name.ends_with(".dat_old") || name.ends_with(".nbt")) {
            if let Ok(tag) = decode_compressed_nbt(&bytes) {
                out.push_str(&format!("NBT {relative} {}\n", canonical_string(&tag)));
                continue;
            }
        }
        let digest: String = Sha1::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        out.push_str(&format!("BIN {relative} {} {digest}\n", bytes.len()));
    }
    out
}

#[test]
#[ignore]
fn dumps_the_upgraded_world() {
    let universe = PathBuf::from(std::env::var("WAFER_FILEFIX_UNIVERSE").expect("WAFER_FILEFIX_UNIVERSE"));
    let level = std::env::var("WAFER_FILEFIX_LEVEL").expect("WAFER_FILEFIX_LEVEL");
    let dump_path = std::env::var("WAFER_FILEFIX_OUT").expect("WAFER_FILEFIX_OUT");
    if let Ok(capabilities) = std::env::var("WAFER_FILEFIX_CAPS") {
        let mut flags = capabilities.split(',').map(|flag| flag == "true");
        let capabilities = super::file_system_capabilities::FileSystemCapabilities::new(flags.next().unwrap(), flags.next().unwrap());
        super::file_fixer_upper::force_file_system_capabilities(Some(capabilities));
    }
    let source = LevelStorageSource::create_default(&universe);
    let mut access = source.validate_and_create_access(&level).unwrap();
    let result = crate::server::main::fix_world_data(&mut access, NotificationManager::default());
    match result {
        Ok(Some(tag)) => println!("LEVELDATA {}", canonical_string(&tag)),
        Ok(None) => println!("LEVELDATA null"),
        Err(_) => println!("FAIL"),
    }
    drop(access);
    std::fs::write(dump_path, dump(&universe.join(level))).unwrap();
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wafer-filefix-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_dat(path: &Path, snbt: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    crate::world::level::storage::write_dat(path, &crate::nbt::parse_fully(snbt).unwrap()).unwrap();
}

fn tree(root: &Path) -> Vec<String> {
    let mut entries = Vec::new();
    collect(root, root, &mut entries);
    let mut names: Vec<String> = entries.into_iter().filter(|(_, _, is_directory)| !is_directory).map(|(relative, _, _)| relative).collect();
    names.sort();
    names
}

#[test]
fn an_old_world_is_upgraded_to_the_current_layout() {
    upgrade_an_old_world("upgrade");
}

#[test]
fn an_old_world_is_upgraded_to_the_current_layout_without_hard_links() {
    use super::file_system_capabilities::FileSystemCapabilities;

    super::file_fixer_upper::force_file_system_capabilities(Some(FileSystemCapabilities::new(true, false)));
    upgrade_an_old_world("upgrade-no-links");
    super::file_fixer_upper::force_file_system_capabilities(None);
}

static FILE_FIXER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one_upgrade_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    FILE_FIXER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn upgrade_an_old_world(name: &str) {
    let _guard = one_upgrade_at_a_time();
    let universe = temp_dir(name);
    let world = universe.join("world");
    let level = "{Data:{DataVersion:4770,version:19133,LevelName:\"Old\",GameType:1,Difficulty:3b,hardcore:1b,LastPlayed:1L,\
        Version:{Id:4770,Name:\"1.21\",Snapshot:0b,Series:\"main\"},Player:{DataVersion:4770,UUID:[I;1,2,3,4],Pos:[0.0d,64.0d,0.0d]},\
        rainTime:5,raining:1b,thunderTime:6,thundering:0b,clearWeatherTime:0,\
        WanderingTraderSpawnDelay:20000,WanderingTraderSpawnChance:30,ScheduledEvents:[],\
        WorldGenSettings:{seed:5L,generate_features:1b,bonus_chest:0b,dimensions:{}}}}";
    write_dat(&world.join("level.dat"), level);
    write_dat(&world.join("level.dat_old"), level);
    write_dat(&world.join("playerdata/00000001-0000-0002-0000-000300000004.dat"), "{DataVersion:4770,Pos:[0.0d,64.0d,0.0d]}");
    write_dat(&world.join("data/idcounts.dat"), "{DataVersion:4770,data:{map:5}}");
    write_dat(&world.join("data/map_5.dat"), "{DataVersion:4770,data:{scale:0b}}");
    write_dat(&world.join("data/command_storage_foo.dat"), "{DataVersion:4770,data:{contents:{}}}");
    write_dat(&world.join("data/villages.dat"), "{DataVersion:4770,data:{}}");
    write_dat(&world.join("DIM-1/data/chunks.dat"), "{DataVersion:4770,data:{tickets:[]}}");
    std::fs::create_dir_all(world.join("generated/ns/structures")).unwrap();
    std::fs::write(world.join("generated/ns/structures/x.nbt"), "x").unwrap();
    std::fs::write(world.join("resources.zip"), "zip").unwrap();
    std::fs::write(world.join("misc.txt"), "keep").unwrap();
    let source = LevelStorageSource::create_default(&universe);
    let mut access = source.validate_and_create_access("world").unwrap();
    let tag = crate::server::main::fix_world_data(&mut access, NotificationManager::default()).ok().flatten().expect("level data");
    drop(access);
    assert_eq!(
        tree(&world),
        vec![
            "data/foo/command_storage.dat",
            "data/minecraft/maps/5.dat",
            "data/minecraft/maps/last_id.dat",
            "data/minecraft/scheduled_events.dat",
            "data/minecraft/wandering_trader.dat",
            "data/minecraft/weather.dat",
            "data/minecraft/world_gen_settings.dat",
            "dimensions/minecraft/the_nether/data/minecraft/chunk_tickets.dat",
            "generated/ns/structure/x.nbt",
            "level.dat",
            "misc.txt",
            "players/data/00000001-0000-0002-0000-000300000004.dat",
            "resourcepacks/resources.zip",
            "session.lock",
        ]
    );
    assert_eq!(crate::nbt::get_data_version(&tag, -1), wafer_data::WORLD_VERSION);
    assert!(!world.join("filefix").exists());
    assert_eq!(std::fs::read_dir(&universe).unwrap().count(), 1);
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn a_failed_upgrade_leaves_the_world_untouched() {
    let _guard = one_upgrade_at_a_time();
    let universe = temp_dir("failure");
    let world = universe.join("world");
    let level = "{Data:{DataVersion:4770,version:19133,LevelName:\"Old\",Version:{Id:4770,Name:\"1.21\",Snapshot:0b,Series:\"main\"}}}";
    write_dat(&world.join("level.dat"), level);
    write_dat(&world.join("data/raids.dat"), "{DataVersion:4770,data:{}}");
    write_dat(&world.join("data/raids_end.dat"), "{DataVersion:4770,data:{}}");
    let before: Vec<String> = tree(&world).into_iter().filter(|name| name != "session.lock").collect();
    let source = LevelStorageSource::create_default(&universe);
    let mut access = source.validate_and_create_access("world").unwrap();
    assert!(crate::server::main::fix_world_data(&mut access, NotificationManager::default()).is_err());
    drop(access);
    let after: Vec<String> = tree(&world).into_iter().filter(|name| name != "session.lock").collect();
    assert_eq!(after, before);
    assert!(!world.join("filefix").exists());
    std::fs::remove_dir_all(universe).unwrap();
}

fn interrupted_world(name: &str) -> PathBuf {
    let universe = temp_dir(name);
    let world = universe.join("world");
    let level = "{Data:{DataVersion:4770,version:19133,LevelName:\"Old\",Version:{Id:4770,Name:\"1.21\",Snapshot:0b,Series:\"main\"}}}";
    write_dat(&world.join("level.dat"), level);
    write_dat(&world.join("data/a.dat"), "{DataVersion:4770,data:{}}");
    write_dat(&world.join("data/b.dat"), "{DataVersion:4770,data:{}}");
    std::fs::create_dir_all(world.join("filefix/new_world/data/minecraft")).unwrap();
    write_dat(&world.join("filefix/new_world/level.dat"), "{Data:{DataVersion:5023}}");
    std::fs::write(
        world.join("filefix/upgrade_in_progress.json"),
        "{\"moves\":[{\"from\":\"data/a.dat\",\"to\":\"data/minecraft/a.dat\"},{\"from\":\"data/b.dat\",\"to\":\"data/minecraft/b.dat\"}]}",
    )
    .unwrap();
    universe
}

fn fix_with_forced_capabilities(universe: &Path, atomic_move: bool, hard_links: bool) -> Result<(), super::file_fix_exception::FileFixFailure> {
    use super::file_system_capabilities::FileSystemCapabilities;

    let _guard = one_upgrade_at_a_time();
    super::file_fixer_upper::force_file_system_capabilities(Some(FileSystemCapabilities::new(atomic_move, hard_links)));
    let source = LevelStorageSource::create_default(universe);
    let mut access = source.validate_and_create_access("world").unwrap();
    let unfixed = access.get_unfixed_data_tag_with_fallback().unwrap();
    let result = crate::util::datafix::data_fixers::get_file_fixer()
        .fix(&mut access, unfixed, &crate::util::worldupdate::upgrade_progress::UpgradeProgress::new(NotificationManager::default()))
        .map(|_| ());
    super::file_fixer_upper::force_file_system_capabilities(None);
    result
}

#[test]
fn an_interrupted_upgrade_is_continued_without_hard_links() {
    let universe = interrupted_world("resume");
    let world = universe.join("world");
    std::fs::rename(world.join("data/a.dat"), world.join("filefix/new_world/data/minecraft/a.dat")).unwrap();
    assert!(fix_with_forced_capabilities(&universe, true, false).is_ok());
    assert_eq!(tree(&world).into_iter().filter(|name| name != "session.lock").collect::<Vec<_>>(), vec!["data/minecraft/a.dat", "data/minecraft/b.dat", "level.dat"]);
    assert!(!world.join("filefix").exists());
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn a_failing_move_is_reverted_and_the_moves_file_removed() {
    use super::file_fix_exception::{FileFixException, FileFixFailure};

    let universe = interrupted_world("revert");
    let world = universe.join("world");
    std::fs::remove_file(world.join("data/b.dat")).unwrap();
    std::fs::create_dir(world.join("data/b.dat")).unwrap();
    let Err(FileFixFailure::Exception(FileFixException::Aborted(aborted))) = fix_with_forced_capabilities(&universe, true, false) else { panic!("expected an aborted upgrade") };
    assert!(aborted.not_reverted_moves().is_empty());
    assert!(aborted.cause.as_ref().unwrap().to_string().starts_with("java.io.IOException: Not a regular file: "));
    assert!(world.join("data/a.dat").is_file());
    assert!(!world.join("filefix").exists());
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn a_move_that_cannot_be_reverted_keeps_the_moves_file() {
    use super::file_fix_exception::{FileFixException, FileFixFailure};

    let universe = interrupted_world("no-revert");
    let world = universe.join("world");
    std::fs::create_dir_all(world.join("filefix/new_world/data/minecraft/b.dat")).unwrap();
    let Err(FileFixFailure::Exception(FileFixException::Aborted(aborted))) = fix_with_forced_capabilities(&universe, true, false) else { panic!("expected an aborted upgrade") };
    assert_eq!(aborted.not_reverted_moves().len(), 1);
    assert!(aborted.cause.is_some());
    assert!(world.join("data/a.dat").is_file());
    assert!(world.join("filefix/upgrade_in_progress.json").is_file());
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn a_file_system_without_atomic_moves_is_rejected() {
    use super::file_fix_exception::{FileFixException, FileFixFailure};

    let universe = interrupted_world("no-atomic");
    let result = fix_with_forced_capabilities(&universe, false, true);
    assert!(matches!(result, Err(FileFixFailure::Exception(FileFixException::AtomicMoveNotSupported(_)))));
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn an_unreadable_moves_file_surfaces_the_unchecked_exception() {
    use super::file_fix_exception::FileFixFailure;

    let universe = interrupted_world("bad-moves");
    let world = universe.join("world");
    std::fs::write(world.join("filefix/upgrade_in_progress.json"), "{\"moves\":[{\"from\":\"../x\",\"to\":\"a.dat\"}]}").unwrap();
    let Err(FileFixFailure::Unchecked(exception)) = fix_with_forced_capabilities(&universe, true, false) else { panic!("expected an unchecked exception") };
    assert_eq!(exception.to_string(), format!("java.lang.IllegalStateException: Illegal path traversal: ..{}x", std::path::MAIN_SEPARATOR));
    assert!(world.join("filefix").exists());
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn a_corrupt_chunk_fails_the_upgrade_with_a_completion_exception() {
    use super::file_fix_exception::FileFixFailure;
    use std::io::{Read, Seek, SeekFrom, Write};

    let _guard = one_upgrade_at_a_time();
    let universe = temp_dir("corrupt-chunk");
    let world = universe.join("world");
    let level = "{Data:{DataVersion:4770,version:19133,LevelName:\"Old\",Version:{Id:4770,Name:\"1.21\",Snapshot:0b,Series:\"main\"}}}";
    write_dat(&world.join("level.dat"), level);
    write_dat(&world.join("data/Monument.dat"), "{data:{Features:{\"[0,0]\":{ChunkX:0,ChunkZ:0,id:\"Monument\",Children:[{id:\"MOP\"}]}}}}");
    std::fs::create_dir_all(world.join("region")).unwrap();
    let region_path = world.join("region/r.0.0.mca");
    let mut region = RegionFile::open(&region_path, false).unwrap();
    region.write(ChunkPos::new(0, 0), &crate::nbt::parse_fully("{DataVersion:1343,Level:{xPos:0,zPos:0}}").unwrap()).unwrap();
    region.close().unwrap();
    drop(region);
    let mut file = std::fs::OpenOptions::new().read(true).write(true).open(&region_path).unwrap();
    let mut header = [0u8; 4];
    file.read_exact(&mut header).unwrap();
    let sector = u32::from_be_bytes(header) >> 8;
    file.seek(SeekFrom::Start(sector as u64 * 4096 + 5)).unwrap();
    file.write_all(&[0xFF; 8]).unwrap();
    drop(file);
    let before = tree(&world);
    let source = LevelStorageSource::create_default(&universe);
    let mut access = source.validate_and_create_access("world").unwrap();
    let unfixed = access.get_unfixed_data_tag_with_fallback().unwrap();
    let result = crate::util::datafix::data_fixers::get_file_fixer().fix(&mut access, unfixed, &crate::util::worldupdate::upgrade_progress::UpgradeProgress::new(NotificationManager::default()));
    match result {
        Err(FileFixFailure::Unchecked(exception)) => assert!(exception.to_string().starts_with("java.util.concurrent.CompletionException: "), "{exception}"),
        other => panic!("{other:?}"),
    }
    drop(access);
    assert_eq!(tree(&world).into_iter().filter(|name| name != "session.lock").collect::<Vec<_>>(), before.into_iter().filter(|name| name != "session.lock").collect::<Vec<_>>());
    assert!(!world.join("filefix").exists());
    std::fs::remove_dir_all(universe).unwrap();
}

#[test]
fn the_copy_on_write_file_system_leaves_the_base_folder_alone() {
    use super::virtualfilesystem::copy_on_write_file_system::CopyOnWriteFileSystem;
    use super::virtualfilesystem::copy_on_write_fs_provider::OpenOptions;
    use super::virtualfilesystem::files;
    use crate::util::java_nio::CopyOption;

    let base = temp_dir("cow");
    std::fs::create_dir_all(base.join("a/b")).unwrap();
    std::fs::write(base.join("a/b/f.txt"), "one").unwrap();
    std::fs::write(base.join("g.txt"), "two").unwrap();
    let tmp = base.join("tmp");
    let skipped = tmp.clone();
    let fs = CopyOnWriteFileSystem::create("cow", &base, &tmp, move |path| path == skipped).unwrap();
    let root = fs.root_path();
    files::move_(&root.resolve_str("g.txt"), &root.resolve_str("a/h.txt"), &[CopyOption::CopyAttributes]).unwrap();
    let mut out = files::new_output_stream(&root.resolve_str("a/b/f.txt"), &OpenOptions::sync_output()).unwrap();
    std::io::Write::write_all(&mut out, b"changed").unwrap();
    drop(out);
    assert!(files::exists(&root.resolve_str("a/h.txt")));
    assert!(!files::exists(&root.resolve_str("g.txt")));
    assert!(files::delete_if_exists(&root.resolve_str("a/b")).is_err());
    assert_eq!(std::fs::read_to_string(base.join("a/b/f.txt")).unwrap(), "one");
    assert!(base.join("g.txt").exists());
    let moves = fs.collect_move_operations(&base.join("out"));
    assert_eq!(moves.copied_files.len(), 1);
    assert_eq!(moves.preexisting_files.len(), 1);
    assert_eq!(moves.directories.len(), 2);
    fs.close().unwrap();
    assert!(!tmp.exists());
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn interrupted_upgrade_moves_round_trip_through_json() {
    use super::virtualfilesystem::file_move::FileMove;

    let from = PathBuf::from("base");
    let to = PathBuf::from("new_world");
    let file_move = FileMove::new(from.join("data").join("a.dat"), to.join("x").join("a.dat"));
    let encoded = file_move.encode(&from, &to).unwrap();
    assert_eq!(encoded["from"], "data/a.dat");
    let element = crate::util::gson::from_json_object(&encoded.to_string()).unwrap().unwrap();
    assert_eq!(FileMove::decode(&element, &from, &to).unwrap(), file_move);
}

#[test]
fn interrupted_upgrade_moves_fail_with_the_vanilla_messages() {
    use super::virtualfilesystem::file_move::FileMove;

    let from = PathBuf::from("base");
    let to = PathBuf::from("new_world");
    let separator = std::path::MAIN_SEPARATOR;
    let decode = |json: &str| {
        let element = crate::util::gson::from_json_object(json).map_err(|message| message.replace("\n", "|"))?;
        FileMove::decode_moves(element.as_ref(), &from, &to).map_err(|message| format!("java.lang.IllegalStateException: {message}"))
    };
    let cases: Vec<(&str, String)> = vec![
        ("{}", "java.lang.IllegalStateException: No key moves in MapLike[{}]".into()),
        ("{\"moves\":5}", "java.lang.IllegalStateException: Not a json array: 5".into()),
        ("{\"moves\":null}", "java.lang.IllegalStateException: No key moves in MapLike[{\"moves\":null}]".into()),
        ("{\"moves\":[5]}", "java.lang.IllegalStateException: Not a JSON object: 5".into()),
        ("{\"moves\":[{}]}", "java.lang.IllegalStateException: No key to in MapLike[{}]; No key from in MapLike[{}]".into()),
        ("{\"moves\":[{\"from\":5,\"to\":\"a\"}]}", "java.lang.IllegalStateException: Not a string: 5".into()),
        ("{\"moves\":[{\"from\":1,\"to\":2}]}", "java.lang.IllegalStateException: Not a string: 2; Not a string: 1".into()),
        ("{\"moves\":[5,6,7]}", "java.lang.IllegalStateException: Not a JSON object: 7; Not a JSON object: 6; Not a JSON object: 5".into()),
        ("{\"moves\":[{\"from\":\"a/../../b\",\"to\":\"b\"}]}", format!("java.lang.IllegalStateException: Illegal path traversal: ..{separator}b")),
        ("{\"moves\":[{\"from\":\".\",\"to\":\"b\"}]}", "java.lang.IllegalStateException: Path is empty: ".into()),
        ("[1,2]", "com.google.gson.JsonSyntaxException: Expected a com.google.gson.JsonObject but was com.google.gson.JsonArray; at path $".into()),
        ("null", "com.google.gson.JsonSyntaxException: Expected a com.google.gson.JsonObject but was com.google.gson.JsonNull; at path $".into()),
        (
            "{\"moves\":[{\"from\":\"a\",\"to\":\"b\"}]",
            "com.google.gson.JsonSyntaxException: java.io.EOFException: End of input at line 1 column 33 path $.moves".into(),
        ),
        ("{\"moves\":[{\"from\":\"a\",\"to\":\"b\"}]} x", "com.google.gson.JsonSyntaxException: com.google.gson.stream.MalformedJsonException: Use JsonReader.setStrictness(Strictness.LENIENT) to accept malformed JSON at line 1 column 36 path $|See https://github.com/google/gson/blob/main/Troubleshooting.md#malformed-json".into()),
    ];
    for (json, expected) in cases {
        assert_eq!(decode(json).unwrap_err(), expected, "{json}");
    }
    assert_eq!(decode("{moves:[{from:a,to:b}]}").unwrap(), vec![FileMove::new(from.join("a"), to.join("b"))]);
    assert_eq!(decode("{'moves':[]}").unwrap(), Vec::new());
    assert_eq!(crate::util::gson::from_json_object(""), Ok(None));
    assert_eq!(FileMove::decode_moves(None, &from, &to), Err("Not a JSON object: null".to_string()));
}
