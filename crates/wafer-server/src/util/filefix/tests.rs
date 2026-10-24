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
                    if let Some(tag) = region.read(pos).unwrap() {
                        out.push_str(&format!("CHUNK {} {} {}\n", pos.x, pos.z, canonical_string(&tag)));
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
    let universe = temp_dir("upgrade");
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
    assert_eq!(FileMove::decode(&encoded, &from, &to).unwrap(), file_move);
    let escaping = serde_json::json!({"from": "../x", "to": "a.dat"});
    assert!(FileMove::decode(&escaping, &from, &to).is_err());
}
