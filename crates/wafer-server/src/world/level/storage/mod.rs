pub mod command_storage;
pub mod data_version;
pub mod level_storage_source;
pub mod level_summary;
pub mod level_version;
pub mod loot;
pub mod player_data_storage;
pub mod saved_data_storage;
pub mod tag_value_input;

use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::datafix::data_fixers::get_file_fixer;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use wafer_core::BlockPos;
use wafer_data::{VERSION_NAME, WORLD_VERSION};
use wafer_protocol::nbt::Nbt;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::server::level::game_mode::GameType;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;
use crate::server::log_save_failure;
use crate::world::difficulty::Difficulty;
use crate::util::java_throwable::JavaThrowable;
use crate::world::level::chunk::storage::{WRITE_COMPRESSED_PATH, decode_nbt, encode_nbt, storage_exception};
use crate::util::lifecycle::Lifecycle;
use crate::world::level::levelgen::world_dimensions::SpecialWorldProperty;
use crate::world::level::levelgen::world_gen_settings::WorldGenSettings;
use crate::world::level::saveddata::weather_data::WeatherData;
use crate::world::level::validation::directory_validator::{DirectoryValidator, SymlinkTargetAllowList};
use crate::world::level::validation::path_allow_list::PathAllowList;
use crate::world::level::world_data_configuration::WorldDataConfiguration;

const LEVEL_NBT_VERSION: i32 = 19133;

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

pub fn read_dat(path: &Path) -> io::Result<Option<Nbt>> {
    let Ok(file) = File::open(path) else { return Ok(None) };
    let mut bytes = Vec::new();
    GzDecoder::new(file).read_to_end(&mut bytes)?;
    decode_nbt(&bytes).map(Some)
}

pub fn read_saved_data_dat(path: &Path, data_fix_type: crate::util::datafix::data_fix_types::DataFixTypes) -> io::Result<Option<Nbt>> {
    if !path.exists() {
        return Ok(None);
    }
    read_tag_from_disk(path, data_fix_type, WORLD_VERSION).map(Some)
}

pub fn read_tag_from_disk(data_file: &Path, data_fix_type: DataFixTypes, new_version: i32) -> io::Result<Nbt> {
    let bytes = fs::read(data_file)?;
    let tag = if is_gzip(&bytes) {
        let mut decompressed = Vec::new();
        GzDecoder::new(bytes.as_slice()).read_to_end(&mut decompressed)?;
        decode_nbt(&decompressed)?
    } else {
        decode_nbt(&bytes)?
    };
    let version = crate::nbt::get_data_version(&tag, 1343);
    crate::util::catch_exception(|| data_fix_type.update_tag(tag, version, new_version)).map_err(io::Error::other)
}

fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && (bytes[0] as i32 | (bytes[1] as i32) << 8) == 35615
}

pub fn read_saved_data<T>(file: &Path, thread: &str, id: &str, data_fix_type: DataFixTypes, decode: impl FnOnce(&Nbt) -> crate::util::codec::DecodeResult<T>) -> Option<T> {
    match read_saved_data_dat(file, data_fix_type) {
        Ok(Some(tag)) => {
            let decoded = crate::util::codec::with_nbt_ops(|| match tag.get("data") {
                Some(data) => decode(data),
                None => crate::util::codec::DecodeResult::error("Not a map: null"),
            });
            if let Some(error) = decoded.error {
                error!(thread: thread, "Failed to parse saved data for 'SavedDataType[minecraft:{id}]': {error}");
            }
            decoded.value
        }
        Ok(None) => None,
        Err(_) => {
            error!(thread: thread, "Error loading saved data: SavedDataType[minecraft:{id}]");
            None
        }
    }
}

pub fn write_dat(path: &Path, nbt: &Nbt) -> io::Result<()> {
    if saved_data_storage::collect_dirty_tag(path, nbt) {
        return Ok(());
    }
    fs::create_dir_all(path.parent().unwrap())?;
    write_compressed(nbt, path, &[])
}

const READ_LEVEL_DATA_TAG_RAW: &str = "net.minecraft.world.level.storage.LevelStorageSource.readLevelDataTagRaw(LevelStorageSource.java:269)";
const GET_UNFIXED_DATA_TAG: &str = "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTag(LevelStorageSource.java:513)";
const GET_UNFIXED_DATA_TAG_WITH_FALLBACK_PRIMARY: &str =
    "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:500)";
const GET_UNFIXED_DATA_TAG_WITH_FALLBACK_FALLBACK: &str =
    "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:504)";

pub(crate) fn read_level_data_tag_raw(data_file: &Path) -> Result<Nbt, JavaThrowable> {
    crate::nbt::nbt_io::read_compressed(data_file).map_err(|mut throwable| {
        throwable.push_frame(READ_LEVEL_DATA_TAG_RAW);
        throwable
    })
}

pub(crate) fn get_unfixed_data_tag(data_file: &Path, callers: &[&'static str]) -> Result<Nbt, JavaThrowable> {
    read_level_data_tag_raw(data_file).map_err(|mut throwable| {
        throwable.push_frame(GET_UNFIXED_DATA_TAG);
        throwable.push_frames(callers);
        throwable
    })
}

pub(crate) fn read_compressed(file: &Path) -> io::Result<Nbt> {
    let mut bytes = Vec::new();
    GzDecoder::new(File::open(file)?).read_to_end(&mut bytes)?;
    decode_nbt(&bytes)
}

const WRITE_COMPRESSED_OPEN: &str = "net.minecraft.nbt.NbtIo.writeCompressed(NbtIo.java:70)";

pub(crate) fn write_compressed(tag: &Nbt, file: &Path, callers: &[&'static str]) -> io::Result<()> {
    let output = fs::OpenOptions::new().write(true).create(true).truncate(true).open(file).map_err(|error| {
        let kind = error.kind();
        let mut throwable = storage_exception::new_output_stream_failure(error, file);
        throwable.stack_trace.push(WRITE_COMPRESSED_OPEN);
        throwable.stack_trace.extend_from_slice(callers.get(WRITE_COMPRESSED_PATH.len()..).unwrap_or_default());
        throwable.stack_trace.extend(crate::server::main_thread_frames::current());
        io::Error::new(kind, throwable)
    })?;
    let mut gz = GzEncoder::new(output, Compression::default());
    gz.write_all(&encode_nbt(tag, callers))?;
    gz.finish()?.sync_all()
}

pub(crate) fn create_temp_file(dir: &Path, prefix: &str, suffix: &str) -> io::Result<PathBuf> {
    loop {
        let path = dir.join(format!("{prefix}{}{suffix}", rand::random::<u64>()));
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

const SAVE_LEVEL_DATA_CALLERS: [&str; 3] = [WRITE_COMPRESSED_PATH[0], WRITE_COMPRESSED_PATH[1], "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.saveLevelData(LevelStorageSource.java:546)"];

fn save_level_dat(dir: &WorldDir, root: &Nbt) -> io::Result<()> {
    fs::create_dir_all(&dir.root)?;
    let data_file = create_temp_file(&dir.root, "level", ".dat")?;
    write_compressed(root, &data_file, &SAVE_LEVEL_DATA_CALLERS)?;
    crate::util::safe_replace_file(&dir.level_dat(), &data_file, &dir.old_level_dat());
    Ok(())
}

pub const ALLOWED_SYMLINKS_CONFIG_NAME: &str = "allowed_symlinks.txt";

pub fn parse_validator(config_path: &Path) -> DirectoryValidator {
    if config_path.exists() {
        let list = fs::read(config_path)
            .map_err(|e| crate::server::dedicated::settings::java_io_exception(&e, config_path))
            .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "java.io.UncheckedIOException: java.nio.charset.MalformedInputException: Input length = 1".to_string()))
            .and_then(|text| PathAllowList::read_plain(&text));
        match list {
            Ok(list) => return DirectoryValidator::new(SymlinkTargetAllowList::PathAllowList(list)),
            Err(e) => error!("Failed to parse {ALLOWED_SYMLINKS_CONFIG_NAME}, disallowing all symbolic links\n{e}"),
        }
    }
    DirectoryValidator::new(SymlinkTargetAllowList::Nothing)
}

pub struct WorldDir {
    pub root: PathBuf,
}

impl WorldDir {
    pub fn level_dat(&self) -> PathBuf {
        self.root.join("level.dat")
    }
    pub fn old_level_dat(&self) -> PathBuf {
        self.root.join("level.dat_old")
    }
    pub fn corrupted_level_dat(&self) -> PathBuf {
        self.root.join(format!("level.dat_corrupted_{}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")))
    }
    pub fn has_world_data(&self) -> bool {
        self.level_dat().exists() || self.old_level_dat().exists()
    }
    pub fn unfixed_data_tag_with_fallback(&self, callers: &[&'static str]) -> Result<Nbt, JavaThrowable> {
        let primary_callers: Vec<&'static str> = [GET_UNFIXED_DATA_TAG_WITH_FALLBACK_PRIMARY].into_iter().chain(callers.iter().copied()).collect();
        match get_unfixed_data_tag(&self.level_dat(), &primary_callers) {
            Ok(root) => Ok(root),
            Err(e) if e.is_io_exception() || e.is_nbt_exception() => {
                warn!("Failed to load world data from {}\n{}", self.level_dat().display(), e.print_stack_trace().trim_end());
                info!("Attempting to use fallback {}", self.old_level_dat().display());
                let fallback_callers: Vec<&'static str> = [GET_UNFIXED_DATA_TAG_WITH_FALLBACK_FALLBACK].into_iter().chain(callers.iter().copied()).collect();
                let root = get_unfixed_data_tag(&self.old_level_dat(), &fallback_callers)?;
                self.restore_level_data_from_old();
                Ok(root)
            }
            Err(e) => Err(e),
        }
    }
    fn restore_level_data_from_old(&self) -> bool {
        crate::util::safe_replace_or_move_file(&self.level_dat(), &self.old_level_dat(), &self.corrupted_level_dat(), true)
    }
    pub fn player_advancements(&self, uuid: u128) -> PathBuf {
        self.root.join("players/advancements").join(format!("{}.json", format_uuid(uuid)))
    }
    pub fn icon_file(&self) -> PathBuf {
        self.root.join("icon.png")
    }
    pub fn data(&self, name: &str) -> PathBuf {
        self.root.join("data").join("minecraft").join(format!("{name}.dat"))
    }
    pub fn generated(&self) -> PathBuf {
        self.root.join("generated")
    }

    pub fn data_folder(&self) -> PathBuf {
        self.root.join("data")
    }
    pub fn regions(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("region")
    }
    pub fn dimension_data_folder(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("data")
    }
    pub fn entities(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("entities")
    }
    pub fn poi(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("poi")
    }
    pub fn player(&self, uuid: u128) -> PathBuf {
        self.root.join("players/data").join(format!("{}.dat", format_uuid(uuid)))
    }
    pub fn player_stats_dir(&self) -> PathBuf {
        self.root.join("players").join("stats")
    }
}

pub fn format_uuid(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn uuid_ints(uuid: u128) -> Nbt {
    Nbt::IntArray((0..4).map(|i| (uuid >> (96 - 32 * i)) as u32 as i32).collect())
}

fn compound(fields: Vec<(&str, Nbt)>) -> Nbt {
    Nbt::Compound(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn string(s: &str) -> Nbt {
    Nbt::String(s.to_owned())
}

fn data_file(data: Nbt) -> Nbt {
    compound(vec![("data", data), ("DataVersion", Nbt::Int(WORLD_VERSION))])
}

#[derive(Clone, Debug)]
pub struct RespawnData {
    pub dimension: String,
    pub pos: BlockPos,
    pub yaw: f32,
    pub pitch: f32,
}

impl PartialEq for RespawnData {
    fn eq(&self, other: &RespawnData) -> bool {
        self.dimension == other.dimension && self.pos == other.pos && self.yaw.to_bits() == other.yaw.to_bits() && self.pitch.to_bits() == other.pitch.to_bits()
    }
}

impl RespawnData {
    pub fn of(dimension: &str, pos: BlockPos, yaw: f32, pitch: f32) -> RespawnData {
        RespawnData { dimension: dimension.to_string(), pos, yaw: wafer_core::mth::wrap_degrees(yaw), pitch: wafer_core::mth::clamp_f(pitch, -90.0, 90.0) }
    }

    pub fn to_nbt_fields(&self) -> Vec<(String, Nbt)> {
        vec![
            ("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z])),
            ("pitch".into(), Nbt::Float(self.pitch)),
            ("dimension".into(), string(&self.dimension)),
            ("yaw".into(), Nbt::Float(self.yaw)),
        ]
    }

    pub fn decode_map(tag: &Nbt) -> DecodeResult<RespawnData> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let dimension = record.required(tag, "dimension", |dimension| codec::identifier(dimension));
        let pos = record.required(tag, "pos", extra_codecs::block_pos);
        let yaw = record.required(tag, "yaw", |yaw| decode_float_range(yaw, -180.0, 180.0));
        let pitch = record.required(tag, "pitch", |pitch| decode_float_range(pitch, -90.0, 90.0));
        record.build(|| Some(RespawnData { dimension: dimension?, pos: pos?, yaw: yaw?, pitch: pitch? }))
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<RespawnData> {
        let Some(Nbt::IntArray(p)) = nbt.get("pos") else { return None };
        let [x, y, z] = p[..] else { return None };
        let dimension = nbt.get("dimension").and_then(Nbt::as_str)?.to_string();
        let angle = |key: &str| nbt.get(key).and_then(Nbt::as_f64).map(|v| v as f32);
        Some(RespawnData { dimension, pos: BlockPos::new(x, y, z), yaw: angle("yaw")?, pitch: angle("pitch")? })
    }
}

fn decode_float_range(tag: &Nbt, min: f32, max: f32) -> DecodeResult<f32> {
    let value = match codec::float(tag) {
        Ok(value) => value,
        Err(error) => return DecodeResult::error(error),
    };
    if value >= min && value <= max {
        return DecodeResult::success(value);
    }
    DecodeResult {
        value: Some(value),
        error: Some(format!("Value {} outside of range [{}:{}]", crate::nbt::java_float_to_string(value), crate::nbt::java_float_to_string(min), crate::nbt::java_float_to_string(max))),
    }
}

pub struct WorldData {
    pub game_time: i64,
    pub spawn: RespawnData,
    pub game_type: GameType,
    pub difficulty: Difficulty,
    pub difficulty_locked: bool,
    pub hardcore: bool,
    pub clocks: Nbt,
    pub weather: WeatherData,
    pub special_world_property: SpecialWorldProperty,
    pub world_gen_settings_lifecycle: Lifecycle,
    pub initialized: bool,
    pub server_brands: Vec<String>,
    pub was_modded: bool,
    pub removed_features: crate::util::java_collections::JavaHashSet,
    pub game_rules: Nbt,
    pub data_configuration: WorldDataConfiguration,
    pub level_name: String,
    pub allow_commands: bool,
    pub singleplayer_uuid: Option<Nbt>,
}

impl WorldData {
    pub fn new_world(
        game_type: GameType,
        difficulty: Difficulty,
        hardcore: bool,
        data_configuration: WorldDataConfiguration,
        level_name: String,
        special_world_property: SpecialWorldProperty,
        world_gen_settings_lifecycle: Lifecycle,
    ) -> WorldData {
        WorldData {
            game_time: 0,
            spawn: RespawnData::of("minecraft:overworld", BlockPos::ZERO, 0.0, 0.0),
            game_type,
            difficulty,
            difficulty_locked: false,
            hardcore,
            clocks: Nbt::Compound(Vec::new().into()),
            weather: WeatherData::default(),
            special_world_property,
            world_gen_settings_lifecycle,
            initialized: false,
            server_brands: Vec::new(),
            was_modded: false,
            removed_features: Default::default(),
            game_rules: Nbt::Compound(Vec::new().into()),
            data_configuration,
            level_name,
            allow_commands: false,
            singleplayer_uuid: None,
        }
    }

    pub fn write_last_played(tag: Dynamic) -> Dynamic {
        let last_played = tag.create_long(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64);
        tag.set("LastPlayed", last_played)
    }

    pub fn write_version_tag(tag: Dynamic) -> Dynamic {
        let world_version = tag
            .empty_map()
            .set("Name", tag.create_string(VERSION_NAME))
            .set("Id", tag.create_int(WORLD_VERSION))
            .set("Snapshot", tag.create_boolean(false))
            .set("Series", tag.create_string("main"));
        tag.set("Version", world_version)
    }

    pub fn load_level_tag(dir: &WorldDir) -> io::Result<Option<Nbt>> {
        if !dir.has_world_data() {
            return Ok(None);
        }
        let level = dir.unfixed_data_tag_with_fallback(&[]).map_err(io::Error::other)?;
        let level = level.get("Data").ok_or_else(|| invalid("level.dat has no Data"))?;
        let data_version = level.get("DataVersion").and_then(Nbt::as_i64).unwrap_or(-1) as i32;
        if get_file_fixer().requires_file_fixing(data_version) {
            return Err(invalid("Cannot get level data without file fixing first"));
        }
        DataFixTypes::Level.update_to_current_version(Dynamic::new(level.clone()), data_version).into_value().map(Some).ok_or_else(|| invalid("level.dat has no Data"))
    }

    pub fn from_tag(level: &Nbt, data_configuration: WorldDataConfiguration, special_world_property: SpecialWorldProperty, world_gen_settings_lifecycle: Lifecycle) -> WorldData {
        let spawn = level.get("spawn").and_then(RespawnData::from_nbt).unwrap_or_else(|| RespawnData::of("minecraft:overworld", BlockPos::ZERO, 0.0, 0.0));
        let difficulty_settings = level.get("difficulty_settings");
        let setting = |key: &str| difficulty_settings.and_then(|d| d.get(key));
        let flag = |key: &str| setting(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        let game_type = GameType::by_id(level.get("GameType").and_then(Nbt::as_i64).unwrap_or(0) as i32);
        WorldData {
            game_time: level.get("Time").and_then(Nbt::as_i64).unwrap_or(0),
            spawn,
            game_type,
            difficulty: setting("difficulty").and_then(Nbt::as_str).and_then(Difficulty::by_name).unwrap_or(Difficulty::Normal),
            difficulty_locked: flag("locked"),
            hardcore: flag("hardcore"),
            clocks: Nbt::Compound(Vec::new().into()),
            weather: WeatherData::default(),
            special_world_property,
            world_gen_settings_lifecycle,
            initialized: level.get("initialized").and_then(Nbt::as_i64).is_none_or(|v| v != 0),
            server_brands: level.get("ServerBrands").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(Nbt::as_str).fold(Vec::new(), |mut brands, brand| {
                if !brands.iter().any(|b| b == brand) {
                    brands.push(brand.to_string());
                }
                brands
            }),
            was_modded: level.get("WasModded").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
            removed_features: level.get("removed_features").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(Nbt::as_str).fold(Default::default(), |mut features: crate::util::java_collections::JavaHashSet, feature| {
                features.insert(feature.to_string());
                features
            }),
            game_rules: Nbt::Compound(Vec::new().into()),
            data_configuration,
            level_name: level.get("LevelName").and_then(Nbt::as_str).unwrap_or("").to_string(),
            allow_commands: level.get("allowCommands").and_then(Nbt::as_i64).map_or(game_type == GameType::Creative, |v| v != 0),
            singleplayer_uuid: level.get("singleplayer_uuid").filter(|uuid| crate::world::entity::entity_save::decode_uuid(uuid).into_result().is_ok()).cloned(),
        }
    }

    pub fn save_level_data(&self, dir: &WorldDir) -> io::Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        let mut data = vec![
            (
                "difficulty_settings",
                compound(vec![
                    ("difficulty", string(self.difficulty.key())),
                    ("hardcore", Nbt::Byte(self.hardcore as i8)),
                    ("locked", Nbt::Byte(self.difficulty_locked as i8)),
                ]),
            ),
            ("Time", Nbt::Long(self.game_time)),
            ("version_history", Nbt::List(vec![Nbt::Int(WORLD_VERSION)])),
            ("GameType", Nbt::Int(self.game_type.id())),
            ("ServerBrands", Nbt::List(self.server_brands.iter().map(|b| string(b)).collect())),
            ("version", Nbt::Int(LEVEL_NBT_VERSION)),
            ("LastPlayed", Nbt::Long(now)),
            ("spawn", Nbt::Compound(self.spawn.to_nbt_fields().into())),
            (
                "Version",
                compound(vec![
                    ("Snapshot", Nbt::Byte(0)),
                    ("Series", string("main")),
                    ("Id", Nbt::Int(WORLD_VERSION)),
                    ("Name", string(VERSION_NAME)),
                ]),
            ),
            ("LevelName", string(&self.level_name)),
            ("initialized", Nbt::Byte(self.initialized as i8)),
            ("WasModded", Nbt::Byte(self.was_modded as i8)),
            ("DataVersion", Nbt::Int(WORLD_VERSION)),
            ("allowCommands", Nbt::Byte(self.allow_commands as i8)),
            ("DataPacks", self.data_configuration.data_packs.to_nbt()),
        ];
        if let Some(enabled_features) = self.data_configuration.encoded_enabled_features() {
            let after_version = data.iter().position(|(key, _)| *key == "version").unwrap() + 1;
            data.insert(after_version, ("enabled_features", enabled_features));
        }
        if let Some(singleplayer_uuid) = &self.singleplayer_uuid {
            data.insert(1, ("singleplayer_uuid", singleplayer_uuid.clone()));
        }
        if !self.removed_features.is_empty() {
            let after_was_modded = data.iter().position(|(key, _)| *key == "WasModded").unwrap() + 1;
            data.insert(after_was_modded, ("removed_features", Nbt::List(self.removed_features.iter().map(string).collect())));
            let keys: Vec<&str> = data.iter().map(|(key, _)| *key).collect();
            let order = wafer_protocol::nbt::java_hash_map_order(&keys);
            let mut slots: Vec<Option<(&str, Nbt)>> = data.into_iter().map(Some).collect();
            data = order.into_iter().map(|index| slots[index].take().unwrap()).collect();
        }
        let level = compound(vec![("Data", compound(data))]);
        save_level_dat(dir, &level)
    }

    pub fn save_saved_data(&self, dir: &WorldDir, gen_settings: &WorldGenSettings, dirty: DirtySavedData) {
        if dirty.world_gen_settings {
            log_save_failure("world_gen_settings.dat", write_dat(&dir.data("world_gen_settings"), &data_file(gen_settings.encode())));
        }
        if dirty.world_clocks {
            log_save_failure("world_clocks.dat", write_dat(&dir.data("world_clocks"), &data_file(self.clocks.clone())));
        }
        if dirty.weather {
            log_save_failure("weather.dat", write_dat(&dir.data("weather"), &data_file(self.weather.to_nbt())));
        }
        if dirty.game_rules {
            log_save_failure("game_rules.dat", write_dat(&dir.data("game_rules"), &data_file(self.game_rules.clone())));
        }
    }
}

#[derive(Clone, Copy)]
pub struct DirtySavedData {
    pub world_gen_settings: bool,
    pub world_clocks: bool,
    pub weather: bool,
    pub game_rules: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_features_are_saved_in_hash_map_order() {
        let root = std::env::temp_dir().join(format!("wafer-removed-features-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = WorldDir { root: root.clone() };
        let mut data = WorldData::new_world(
            GameType::Survival,
            Difficulty::Normal,
            false,
            WorldDataConfiguration::default_configuration(),
            "world".into(),
            crate::world::level::levelgen::world_dimensions::SpecialWorldProperty::None,
            crate::util::lifecycle::Lifecycle::Stable,
        );
        let keys = |dir: &WorldDir| match read_compressed(&dir.level_dat()).unwrap().get("Data") {
            Some(Nbt::Compound(fields)) => fields.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
            _ => panic!("no Data"),
        };
        data.save_level_data(&dir).unwrap();
        let plain = keys(&dir);
        assert_eq!(wafer_protocol::nbt::java_hash_map_order(&plain), (0..plain.len()).collect::<Vec<_>>());
        data.removed_features.insert("minecraft:trade_rebalance".into());
        data.save_level_data(&dir).unwrap();
        let with_removed = keys(&dir);
        assert!(with_removed.iter().any(|key| key == "removed_features"));
        assert_eq!(wafer_protocol::nbt::java_hash_map_order(&with_removed), (0..with_removed.len()).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn uuid_format() {
        assert_eq!(format_uuid(0xd76279edcc573f1885f4367c1defa1ee), "d76279ed-cc57-3f18-85f4-367c1defa1ee");
    }
}

#[cfg(test)]
mod player_data_storage_tests;
