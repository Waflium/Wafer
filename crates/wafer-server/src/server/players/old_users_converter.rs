use wafer_core::java_string::JavaStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use wafer_core::random::java_string_hash;

use super::cached_user_name_to_id_resolver::CachedUserNameToIdResolver;
use super::{Ban, IpBanEntry, LoadError, NameAndId, OpEntry, StoredUserList, UserBanEntry, WhiteListEntry};
use crate::commands::arguments::java_uuid_from_string;
use crate::network::login::offline_uuid;
use crate::server::services::Services;
use crate::util::java_collections::JavaHashMap;
use crate::util::java_nio::{java_io_error, os_message};
use crate::util::java_throwable::JavaThrowable;
use crate::util::string_util::java_trim;
use crate::world::level::storage::format_uuid;

pub const OLD_IPBANLIST: &str = "banned-ips.txt";
pub const OLD_USERBANLIST: &str = "banned-players.txt";
pub const OLD_OPLIST: &str = "ops.txt";
pub const OLD_WHITELIST: &str = "white-list.txt";
const USERBANLIST_FILE: &str = "banned-players.json";
const IPBANLIST_FILE: &str = "banned-ips.json";
const OPLIST_FILE: &str = "ops.json";
const WHITELIST_FILE: &str = "whitelist.json";
const RETRY_DELAY: Duration = Duration::from_millis(5000);

#[derive(Clone)]
pub struct UserLookup {
    pub name_to_id_cache: Arc<CachedUserNameToIdResolver>,
    pub services: Services,
    pub uses_authentication: bool,
}

#[derive(Debug)]
pub struct ConversionError {
    message: String,
    cause: Option<String>,
}

impl ConversionError {
    pub fn new(message: impl Into<String>) -> ConversionError {
        ConversionError { message: message.into(), cause: None }
    }

    fn with_cause(message: impl Into<String>, cause: &LookupException) -> ConversionError {
        ConversionError { message: message.into(), cause: Some(cause.to_string()) }
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "net.minecraft.server.players.OldUsersConverter$ConversionError: {}", self.message)?;
        match &self.cause {
            Some(cause) => write!(f, "\nCaused by: {cause}"),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookupException {
    ProfileNotFound(String),
    AuthenticationUnavailable,
    Authentication(String),
}

impl LookupException {
    pub fn is_profile_not_found(&self) -> bool {
        matches!(self, LookupException::ProfileNotFound(_))
    }
}

impl fmt::Display for LookupException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupException::ProfileNotFound(message) => write!(f, "com.mojang.authlib.services.ProfileNotFoundException: {message}"),
            LookupException::AuthenticationUnavailable => f.write_str("com.mojang.authlib.exceptions.AuthenticationUnavailableException"),
            LookupException::Authentication(cause) => write!(f, "com.mojang.authlib.exceptions.AuthenticationException: {cause}"),
        }
    }
}

pub trait ProfileLookupCallback {
    fn on_profile_lookup_succeeded(&mut self, profile_name: &str, profile_id: u128) -> Result<(), ConversionError>;

    fn on_profile_lookup_failed(&mut self, profile_name: &str, exception: &LookupException) -> Result<(), ConversionError>;
}

pub struct OldUsersConverter<'a> {
    pub server_directory: &'a Path,
    pub world_directory: Option<&'a Path>,
    pub services: &'a Services,
    pub name_to_id_cache: &'a CachedUserNameToIdResolver,
    pub uses_authentication: bool,
    pub operator_permission_level: i32,
}

type UserMap = JavaHashMap<Vec<String>>;

fn lines_of(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut line));
            }
            '\n' => lines.push(std::mem::take(&mut line)),
            other => line.push(other),
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn read_lines(path: &Path) -> io::Result<Vec<String>> {
    std::fs::read(path).map(|bytes| lines_of(&bytes)).map_err(|e| file_not_found(&e, path))
}

pub fn file_not_found(error: &io::Error, path: &Path) -> io::Error {
    java_io_error(error.kind(), "java.io.FileNotFoundException", format!("{} ({})", path.display(), os_message(error).trim_end_matches('.')))
}

fn split_pipe(line: &str) -> Vec<String> {
    let mut parts: Vec<String> = line.split('|').map(str::to_string).collect();
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

pub enum ConvertFailure {
    Io(io::Error),
    Unchecked(JavaThrowable),
}

impl From<io::Error> for ConvertFailure {
    fn from(error: io::Error) -> ConvertFailure {
        ConvertFailure::Io(error)
    }
}

fn read_old_list_format(path: &Path, user_map: &mut UserMap, tail: &[&'static str]) -> Result<Vec<String>, ConvertFailure> {
    let lines = read_lines(path)?;
    for line in &lines {
        let line = java_trim(line);
        if !line.starts_with('#') && !line.is_empty() {
            let parts = split_pipe(line);
            let Some(name) = parts.first() else {
                let stack_trace = ["net.minecraft.server.players.OldUsersConverter.readOldListFormat(OldUsersConverter.java:47)"].into_iter().chain(tail.iter().copied()).collect();
                return Err(ConvertFailure::Unchecked(JavaThrowable::new("java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0", stack_trace)));
            };
            user_map.insert(name.to_lowercase(), parts);
        }
    }
    Ok(lines)
}

const CONVERT_USER_BANLIST_68: &str = "net.minecraft.server.players.OldUsersConverter.convertUserBanlist(OldUsersConverter.java:68)";
const CONVERT_USER_BANLIST_75: &str = "net.minecraft.server.players.OldUsersConverter.convertUserBanlist(OldUsersConverter.java:75)";
const CONVERT_IP_BANLIST_123: &str = "net.minecraft.server.players.OldUsersConverter.convertIpBanlist(OldUsersConverter.java:123)";
const CONVERT_IP_BANLIST_130: &str = "net.minecraft.server.players.OldUsersConverter.convertIpBanlist(OldUsersConverter.java:130)";
const CONVERT_OPS_LIST_156: &str = "net.minecraft.server.players.OldUsersConverter.convertOpsList(OldUsersConverter.java:156)";
const CONVERT_WHITE_LIST_199: &str = "net.minecraft.server.players.OldUsersConverter.convertWhiteList(OldUsersConverter.java:199)";
const CONVERT_OLD_USERS_622: &str = "net.minecraft.server.dedicated.DedicatedServer.convertOldUsers(DedicatedServer.java:622)";
const CONVERT_OLD_USERS_633: &str = "net.minecraft.server.dedicated.DedicatedServer.convertOldUsers(DedicatedServer.java:633)";
const CONVERT_OLD_USERS_644: &str = "net.minecraft.server.dedicated.DedicatedServer.convertOldUsers(DedicatedServer.java:644)";
const CONVERT_OLD_USERS_655: &str = "net.minecraft.server.dedicated.DedicatedServer.convertOldUsers(DedicatedServer.java:655)";

fn server_tail(converter: &'static str, convert_old_users: &'static str) -> [&'static str; 6] {
    [converter, convert_old_users, crate::server::INIT_SERVER_FRAME, crate::server::RUN_SERVER_FRAME, crate::server::SPIN_FRAME, crate::server::THREAD_RUN_FRAME]
}

fn is_file(path: &Path) -> bool {
    path.is_file()
}

fn parse_date(date_string: &str) -> Option<DateTime<FixedOffset>> {
    let millis = crate::util::java_simple_date_format::parse(date_string)?;
    DateTime::from_timestamp_millis(millis).map(|date| date.fixed_offset())
}

fn user_def_ban(user_def: &[String]) -> Ban {
    let created = user_def.get(1).and_then(|date| parse_date(date));
    let source = user_def.get(2).cloned();
    let expires = user_def.get(3).and_then(|date| parse_date(date));
    let reason = user_def.get(4).cloned();
    Ban::with_created(created, source, expires, reason)
}

fn rename_old_file(path: &Path) {
    let mut converted = path.as_os_str().to_owned();
    converted.push(".converted");
    let _ = std::fs::rename(path, PathBuf::from(converted));
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn load_existing<E: super::StoredUserEntry>(list: &mut StoredUserList<E>, file: &Path, tail: &[&'static str]) -> Result<(), JavaThrowable> {
    if file.exists() {
        match list.load(tail) {
            Ok(()) => {}
            Err(LoadError::Io(e)) => warn!("Could not load existing file {}\n{}", file_name(file), java_load_exception(&e, file)),
            Err(LoadError::Unchecked(throwable)) => return Err(throwable),
        }
    }
    Ok(())
}

fn java_load_exception(error: &io::Error, file: &Path) -> String {
    if error.raw_os_error().is_some() { file_not_found(error, file).to_string() } else { error.to_string() }
}

impl OldUsersConverter<'_> {
    fn resolve(&self, name: &str) -> PathBuf {
        if self.server_directory == Path::new(".") { PathBuf::from(name) } else { self.server_directory.join(name) }
    }

    fn lookup_players<'n>(&self, names: impl IntoIterator<Item = &'n str>, callback: &mut dyn ProfileLookupCallback) -> Result<(), ConversionError> {
        let filtered_names: Vec<&str> = names.into_iter().filter(|name| !name.is_empty()).collect();
        if self.uses_authentication {
            self.services.find_profiles_by_names(&filtered_names, callback)
        } else {
            for name in filtered_names {
                callback.on_profile_lookup_succeeded(name, offline_uuid(name))?;
            }
            Ok(())
        }
    }

    pub fn convert_user_banlist(&self) -> Result<bool, JavaThrowable> {
        let file = self.resolve(USERBANLIST_FILE);
        let mut bans: StoredUserList<UserBanEntry> = StoredUserList::new(Some(file.clone()));
        let old_file = self.resolve(OLD_USERBANLIST);
        if !(old_file.exists() && is_file(&old_file)) {
            return Ok(true);
        }
        load_existing(&mut bans, &file, &server_tail(CONVERT_USER_BANLIST_68, CONVERT_OLD_USERS_622))?;
        let mut user_map = UserMap::new(java_string_hash);
        match read_old_list_format(&old_file, &mut user_map, &server_tail(CONVERT_USER_BANLIST_75, CONVERT_OLD_USERS_622)) {
            Ok(_) => {}
            Err(ConvertFailure::Io(e)) => {
                warn!("Could not read old user banlist to convert it!\n{}", java_io_message(&e));
                return Ok(false);
            }
            Err(ConvertFailure::Unchecked(throwable)) => return Err(throwable),
        }
        let result = {
            let mut callback = UserBanCallback { converter: self, bans: &mut bans, user_map: &user_map };
            let names: Vec<&str> = user_map.iter().map(|(name, _)| name).collect();
            self.lookup_players(names, &mut callback)
        };
        if let Err(e) = result {
            error!("Conversion failed, please try again later\n{e}");
            return Ok(false);
        }
        if let Err(e) = bans.save() {
            warn!("Could not read old user banlist to convert it!\n{}", java_load_exception(&e, &file));
            return Ok(false);
        }
        rename_old_file(&old_file);
        Ok(true)
    }

    pub fn convert_ip_banlist(&self) -> Result<bool, JavaThrowable> {
        let file = self.resolve(IPBANLIST_FILE);
        let mut ip_bans: StoredUserList<IpBanEntry> = StoredUserList::new(Some(file.clone()));
        let old_file = self.resolve(OLD_IPBANLIST);
        if !(old_file.exists() && is_file(&old_file)) {
            return Ok(true);
        }
        load_existing(&mut ip_bans, &file, &server_tail(CONVERT_IP_BANLIST_123, CONVERT_OLD_USERS_633))?;
        let mut user_map = UserMap::new(java_string_hash);
        match read_old_list_format(&old_file, &mut user_map, &server_tail(CONVERT_IP_BANLIST_130, CONVERT_OLD_USERS_633)) {
            Ok(_) => {}
            Err(ConvertFailure::Io(e)) => {
                warn!("Could not parse old ip banlist to convert it!\n{}", java_io_message(&e));
                return Ok(false);
            }
            Err(ConvertFailure::Unchecked(throwable)) => return Err(throwable),
        }
        for (key, user_def) in user_map.iter() {
            ip_bans.add(IpBanEntry { ip: key.to_string(), ban: user_def_ban(user_def) });
        }
        if let Err(e) = ip_bans.save() {
            warn!("Could not parse old ip banlist to convert it!\n{}", java_load_exception(&e, &file));
            return Ok(false);
        }
        rename_old_file(&old_file);
        Ok(true)
    }

    pub fn convert_ops_list(&self) -> Result<bool, JavaThrowable> {
        let file = self.resolve(OPLIST_FILE);
        let mut ops_list: StoredUserList<OpEntry> = StoredUserList::new(Some(file.clone()));
        let old_file = self.resolve(OLD_OPLIST);
        if !(old_file.exists() && is_file(&old_file)) {
            return Ok(true);
        }
        load_existing(&mut ops_list, &file, &server_tail(CONVERT_OPS_LIST_156, CONVERT_OLD_USERS_644))?;
        let lines = match read_lines(&old_file) {
            Ok(lines) => lines,
            Err(e) => {
                warn!("Could not read old oplist to convert it!\n{}", java_io_message(&e));
                return Ok(false);
            }
        };
        let result = {
            let mut callback = OpListCallback { converter: self, ops_list: &mut ops_list };
            self.lookup_players(lines.iter().map(String::as_str), &mut callback)
        };
        if let Err(e) = result {
            error!("Conversion failed, please try again later\n{e}");
            return Ok(false);
        }
        if let Err(e) = ops_list.save() {
            warn!("Could not read old oplist to convert it!\n{}", java_load_exception(&e, &file));
            return Ok(false);
        }
        rename_old_file(&old_file);
        Ok(true)
    }

    pub fn convert_white_list(&self) -> Result<bool, JavaThrowable> {
        let file = self.resolve(WHITELIST_FILE);
        let mut whitelist: StoredUserList<WhiteListEntry> = StoredUserList::new(Some(file.clone()));
        let old_file = self.resolve(OLD_WHITELIST);
        if !(old_file.exists() && is_file(&old_file)) {
            return Ok(true);
        }
        load_existing(&mut whitelist, &file, &server_tail(CONVERT_WHITE_LIST_199, CONVERT_OLD_USERS_655))?;
        let lines = match read_lines(&old_file) {
            Ok(lines) => lines,
            Err(e) => {
                warn!("Could not read old whitelist to convert it!\n{}", java_io_message(&e));
                return Ok(false);
            }
        };
        let result = {
            let mut callback = WhiteListCallback { converter: self, whitelist: &mut whitelist };
            self.lookup_players(lines.iter().map(String::as_str), &mut callback)
        };
        if let Err(e) = result {
            error!("Conversion failed, please try again later\n{e}");
            return Ok(false);
        }
        if let Err(e) = whitelist.save() {
            warn!("Could not read old whitelist to convert it!\n{}", java_load_exception(&e, &file));
            return Ok(false);
        }
        rename_old_file(&old_file);
        Ok(true)
    }

    pub fn convert_players(&self) -> Result<bool, JavaThrowable> {
        let Some(world_directory) = self.world_directory else { return Ok(true) };
        let world_player_directory = world_directory.join("players");
        let world_new_player_directory = world_directory.join("players/data");
        let unknown_player_directory = world_directory.join("unknownplayers");
        if !(world_player_directory.exists() && world_player_directory.is_dir()) {
            return Ok(true);
        }
        let mut player_names: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&world_player_directory).into_iter().flatten().flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.to_lowercase().ends_with(".dat") {
                let player_name = &file_name[..file_name.len() - ".dat".len()];
                if !player_name.is_empty() {
                    player_names.push(player_name.to_string());
                }
            }
        }
        let mut callback = PlayerFilesCallback {
            converter: self,
            world_player_directory: &world_player_directory,
            world_new_player_directory: &world_new_player_directory,
            unknown_player_directory: &unknown_player_directory,
            names: &player_names,
        };
        if let Err(e) = self.lookup_players(player_names.iter().map(String::as_str), &mut callback) {
            error!("Conversion failed, please try again later\n{e}");
            return Ok(false);
        }
        Ok(true)
    }

    pub fn convert_old_users(&self) -> Result<bool, JavaThrowable> {
        let mut user_banlist_converted = false;
        let mut retries = 0;
        while !user_banlist_converted && retries <= 2 {
            if retries > 0 {
                warn!("Encountered a problem while converting the user banlist, retrying in a few seconds");
                std::thread::sleep(RETRY_DELAY);
            }
            user_banlist_converted = self.convert_user_banlist()?;
            retries += 1;
        }
        let mut ip_banlist_converted = false;
        retries = 0;
        while !ip_banlist_converted && retries <= 2 {
            if retries > 0 {
                warn!("Encountered a problem while converting the ip banlist, retrying in a few seconds");
                std::thread::sleep(RETRY_DELAY);
            }
            ip_banlist_converted = self.convert_ip_banlist()?;
            retries += 1;
        }
        let mut op_list_converted = false;
        retries = 0;
        while !op_list_converted && retries <= 2 {
            if retries > 0 {
                warn!("Encountered a problem while converting the op list, retrying in a few seconds");
                std::thread::sleep(RETRY_DELAY);
            }
            op_list_converted = self.convert_ops_list()?;
            retries += 1;
        }
        let mut whitelist_converted = false;
        retries = 0;
        while !whitelist_converted && retries <= 2 {
            if retries > 0 {
                warn!("Encountered a problem while converting the whitelist, retrying in a few seconds");
                std::thread::sleep(RETRY_DELAY);
            }
            whitelist_converted = self.convert_white_list()?;
            retries += 1;
        }
        let mut players_converted = false;
        retries = 0;
        while !players_converted && retries <= 2 {
            if retries > 0 {
                warn!("Encountered a problem while converting the player save files, retrying in a few seconds");
                std::thread::sleep(RETRY_DELAY);
            }
            players_converted = self.convert_players()?;
            retries += 1;
        }
        Ok(user_banlist_converted || ip_banlist_converted || op_list_converted || whitelist_converted || players_converted)
    }

    pub fn are_old_userlists_removed(&self) -> bool {
        let found = |name: &str| {
            let path = self.resolve(name);
            path.exists() && is_file(&path)
        };
        let found_user_banlist = found(OLD_USERBANLIST);
        let found_ip_banlist = found(OLD_IPBANLIST);
        let found_op_list = found(OLD_OPLIST);
        let found_whitelist = found(OLD_WHITELIST);
        if !found_user_banlist && !found_ip_banlist && !found_op_list && !found_whitelist {
            return true;
        }
        warn!("**** FAILED TO START THE SERVER AFTER ACCOUNT CONVERSION!");
        warn!("** please remove the following files and restart the server:");
        if found_user_banlist {
            warn!("* {OLD_USERBANLIST}");
        }
        if found_ip_banlist {
            warn!("* {OLD_IPBANLIST}");
        }
        if found_op_list {
            warn!("* {OLD_OPLIST}");
        }
        if found_whitelist {
            warn!("* {OLD_WHITELIST}");
        }
        false
    }
}

fn java_io_message(error: &io::Error) -> String {
    error.to_string()
}

struct UserBanCallback<'a, 'b> {
    converter: &'a OldUsersConverter<'b>,
    bans: &'a mut StoredUserList<UserBanEntry>,
    user_map: &'a UserMap,
}

impl ProfileLookupCallback for UserBanCallback<'_, '_> {
    fn on_profile_lookup_succeeded(&mut self, profile_name: &str, profile_id: u128) -> Result<(), ConversionError> {
        let profile = NameAndId { id: profile_id, name: profile_name.to_string() };
        self.converter.name_to_id_cache.add(profile.clone());
        let Some(user_def) = self.user_map.get(&profile.name.to_lowercase()) else {
            warn!("Could not convert user banlist entry for {}", profile.name);
            return Err(ConversionError::new("Profile not in the conversionlist"));
        };
        self.bans.add(UserBanEntry { user: profile, ban: user_def_ban(user_def) });
        Ok(())
    }

    fn on_profile_lookup_failed(&mut self, profile_name: &str, exception: &LookupException) -> Result<(), ConversionError> {
        warn!("Could not lookup user banlist entry for {profile_name}\n{exception}");
        if !exception.is_profile_not_found() {
            return Err(ConversionError::with_cause(format!("Could not request user {profile_name} from backend systems"), exception));
        }
        Ok(())
    }
}

struct OpListCallback<'a, 'b> {
    converter: &'a OldUsersConverter<'b>,
    ops_list: &'a mut StoredUserList<OpEntry>,
}

impl ProfileLookupCallback for OpListCallback<'_, '_> {
    fn on_profile_lookup_succeeded(&mut self, profile_name: &str, profile_id: u128) -> Result<(), ConversionError> {
        let profile = NameAndId { id: profile_id, name: profile_name.to_string() };
        self.converter.name_to_id_cache.add(profile.clone());
        self.ops_list.add(OpEntry { user: profile, level: self.converter.operator_permission_level, bypasses_player_limit: false });
        Ok(())
    }

    fn on_profile_lookup_failed(&mut self, profile_name: &str, exception: &LookupException) -> Result<(), ConversionError> {
        warn!("Could not lookup oplist entry for {profile_name}\n{exception}");
        if !exception.is_profile_not_found() {
            return Err(ConversionError::with_cause(format!("Could not request user {profile_name} from backend systems"), exception));
        }
        Ok(())
    }
}

struct WhiteListCallback<'a, 'b> {
    converter: &'a OldUsersConverter<'b>,
    whitelist: &'a mut StoredUserList<WhiteListEntry>,
}

impl ProfileLookupCallback for WhiteListCallback<'_, '_> {
    fn on_profile_lookup_succeeded(&mut self, profile_name: &str, profile_id: u128) -> Result<(), ConversionError> {
        let profile = NameAndId { id: profile_id, name: profile_name.to_string() };
        self.converter.name_to_id_cache.add(profile.clone());
        self.whitelist.add(WhiteListEntry { user: profile });
        Ok(())
    }

    fn on_profile_lookup_failed(&mut self, profile_name: &str, exception: &LookupException) -> Result<(), ConversionError> {
        warn!("Could not lookup user whitelist entry for {profile_name}\n{exception}");
        if !exception.is_profile_not_found() {
            return Err(ConversionError::with_cause(format!("Could not request user {profile_name} from backend systems"), exception));
        }
        Ok(())
    }
}

struct PlayerFilesCallback<'a, 'b> {
    converter: &'a OldUsersConverter<'b>,
    world_player_directory: &'a Path,
    world_new_player_directory: &'a Path,
    unknown_player_directory: &'a Path,
    names: &'a [String],
}

impl PlayerFilesCallback<'_, '_> {
    fn move_player_file(&self, directory: &Path, old_name: &str, new_name: &str) -> Result<(), ConversionError> {
        let old_file_name = self.world_player_directory.join(format!("{old_name}.dat"));
        let new_file_name = directory.join(format!("{new_name}.dat"));
        ensure_directory_exists(directory)?;
        if std::fs::rename(&old_file_name, &new_file_name).is_err() {
            return Err(ConversionError::new(format!("Could not convert file for {old_name}")));
        }
        Ok(())
    }

    fn get_file_name_for_profile(&self, profile_name: &str) -> Result<&str, ConversionError> {
        self.names
            .iter()
            .find(|name| name.to_lowercase() == profile_name.to_lowercase())
            .map(String::as_str)
            .ok_or_else(|| ConversionError::new(format!("Could not find the filename for {profile_name} anymore")))
    }
}

impl ProfileLookupCallback for PlayerFilesCallback<'_, '_> {
    fn on_profile_lookup_succeeded(&mut self, profile_name: &str, profile_id: u128) -> Result<(), ConversionError> {
        let profile = NameAndId { id: profile_id, name: profile_name.to_string() };
        self.converter.name_to_id_cache.add(profile);
        let file_name = self.get_file_name_for_profile(profile_name)?;
        self.move_player_file(self.world_new_player_directory, file_name, &format_uuid(profile_id))
    }

    fn on_profile_lookup_failed(&mut self, profile_name: &str, exception: &LookupException) -> Result<(), ConversionError> {
        warn!("Could not lookup user uuid for {profile_name}\n{exception}");
        if exception.is_profile_not_found() {
            let file_name_for_profile = self.get_file_name_for_profile(profile_name)?;
            self.move_player_file(self.unknown_player_directory, file_name_for_profile, file_name_for_profile)
        } else {
            Err(ConversionError::with_cause(format!("Could not request user {profile_name} from backend systems"), exception))
        }
    }
}

fn ensure_directory_exists(directory: &Path) -> Result<(), ConversionError> {
    let error = || ConversionError::new(format!("Can't create directory {} in world save directory.", file_name(directory)));
    if directory.exists() {
        if !directory.is_dir() {
            return Err(error());
        }
    } else if std::fs::create_dir_all(directory).is_err() {
        return Err(error());
    }
    Ok(())
}

pub fn convert_mob_owner_if_necessary(lookup: &UserLookup, owner: &str) -> Option<u128> {
    if !owner.is_empty() && owner.java_utf16().count() <= 16 {
        if let Some(profile) = lookup.name_to_id_cache.get(&lookup.services, owner) {
            Some(profile.id)
        } else if lookup.uses_authentication {
            let mut callback = MobOwnerCallback { name_to_id_cache: &lookup.name_to_id_cache, profiles: Vec::new() };
            let _ = lookup.services.find_profiles_by_names(&[owner], &mut callback);
            callback.profiles.first().map(|profile| profile.id)
        } else {
            Some(offline_uuid(owner))
        }
    } else {
        java_uuid_from_string(owner)
    }
}

struct MobOwnerCallback<'a> {
    name_to_id_cache: &'a CachedUserNameToIdResolver,
    profiles: Vec<NameAndId>,
}

impl ProfileLookupCallback for MobOwnerCallback<'_> {
    fn on_profile_lookup_succeeded(&mut self, profile_name: &str, profile_id: u128) -> Result<(), ConversionError> {
        let profile = NameAndId { id: profile_id, name: profile_name.to_string() };
        self.name_to_id_cache.add(profile.clone());
        self.profiles.push(profile);
        Ok(())
    }

    fn on_profile_lookup_failed(&mut self, profile_name: &str, exception: &LookupException) -> Result<(), ConversionError> {
        warn!("Could not lookup user whitelist entry for {profile_name}\n{exception}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_split_drops_trailing_empty_strings_like_java() {
        assert_eq!(split_pipe("a|b||"), ["a", "b"]);
        assert_eq!(split_pipe("|empty"), ["", "empty"]);
        assert_eq!(split_pipe("a||c"), ["a", "", "c"]);
        assert_eq!(split_pipe("name"), ["name"]);
    }

    fn names_of(file: &Path, field: &str) -> Vec<String> {
        let list: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        list.as_array().unwrap().iter().map(|entry| entry[field].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn old_lists_are_converted_like_vanilla() {
        let dir = std::env::temp_dir().join(format!("wafer-old-users-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let world = dir.join("world");
        std::fs::create_dir_all(world.join("players")).unwrap();
        let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
        write(
            "banned-players.txt",
            "# Banned players\r\n\r\nNotch|2013-04-01 10:00:00 -0500|Console|Forever|Griefing <b>=it's\r\nSteve||src|2099-01-01 00:00:00 +0900|\r\nALEX|2012-1-2 3:4:5 GMT+01:30|Server|Forever|a|b|c\r\nDupe|2013-04-01 10:00:00 -0500\r\ndupe|2014-04-01 10:00:00 -0500|Other\r\n  Spaced  |bad date||x\r\nTrail|||||\r\n|empty\r\n",
        );
        write("banned-ips.txt", "# IP bans\r\n127.0.0.1|2013-04-01 10:00:00 -0500|Console|Forever|bad ip\r\n10.0.0.2|||2014-01-01 00:00:00 +0000|temp\r\nFE80::1\r\n");
        write("ops.txt", "Rozan\r\n# not a comment\r\n\r\nSecond\nThird\rRozan\r\n");
        write("white-list.txt", "Alice\r\nbob\r\nAlice\r\n#comment\r\n");
        write("banned-players.json", "[{\"uuid\":\"11111111-1111-1111-1111-111111111111\",\"name\":\"Existing\",\"created\":\"2013-04-01 10:00:00 -0500\",\"source\":\"x\",\"expires\":\"forever\",\"reason\":\"old\"}]");
        write("ops.json", "[{\"uuid\":\"22222222-2222-2222-2222-222222222222\",\"name\":\"ExistingOp\",\"level\":2,\"bypassesPlayerLimit\":true}]");
        for name in ["Alice.dat", "bob.dat", "Charlie.dat", ".dat", "notes.txt"] {
            std::fs::write(world.join("players").join(name), name).unwrap();
        }
        let services = Services::offline();
        let cache = CachedUserNameToIdResolver::load(Some(dir.clone()));
        let converter = OldUsersConverter {
            server_directory: &dir,
            world_directory: Some(&world),
            services: &services,
            name_to_id_cache: &cache,
            uses_authentication: false,
            operator_permission_level: 3,
        };
        assert!(converter.convert_old_users().unwrap());
        assert!(converter.are_old_userlists_removed());
        assert_eq!(names_of(&dir.join("banned-players.json"), "name"), ["trail", "notch", "alex", "spaced  ", "dupe", "Existing", "steve"]);
        assert_eq!(names_of(&dir.join("banned-players.json"), "source"), ["(Unknown)", "Console", "Server", "", "Other", "x", "src"]);
        let bans = std::fs::read_to_string(dir.join("banned-players.json")).unwrap();
        assert!(bans.contains("\"uuid\": \"42653081-a90e-3475-b3d6-3550cdb43f8e\""));
        assert!(bans.contains("\"reason\": \"Griefing \\u003cb\\u003e\\u003dit\\u0027s\""));
        assert_eq!(names_of(&dir.join("banned-ips.json"), "ip"), ["10.0.0.2", "fe80::1", "127.0.0.1"]);
        assert_eq!(names_of(&dir.join("ops.json"), "name"), ["Third", "Rozan", "Second", "ExistingOp", "# not a comment"]);
        let ops: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("ops.json")).unwrap()).unwrap();
        assert_eq!(ops.as_array().unwrap().iter().map(|op| op["level"].as_i64().unwrap()).collect::<Vec<_>>(), [3, 3, 3, 2, 3]);
        assert_eq!(names_of(&dir.join("whitelist.json"), "name"), ["Alice", "bob", "#comment"]);
        assert_eq!(
            names_of(&dir.join("usercache.json"), "name"),
            ["Charlie", "bob", "Alice", "#comment", "Rozan", "Third", "Second", "# not a comment", "notch", "dupe", "steve", "spaced  ", "alex", "trail"]
        );
        for name in ["banned-players.txt", "banned-ips.txt", "ops.txt", "white-list.txt"] {
            assert!(!dir.join(name).exists());
            assert!(dir.join(format!("{name}.converted")).is_file());
        }
        let mut moved: Vec<String> = std::fs::read_dir(world.join("players/data")).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        moved.sort();
        assert_eq!(moved, ["10920508-d5d8-3eed-93d2-92f193afe7d7.dat", "8e289159-2034-3a16-96b9-9fa637848b3b.dat", "e273e680-80d6-36e2-987c-d622a7d84456.dat"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_blocked_list_file_fails_the_conversion_and_keeps_the_old_list() {
        let dir = std::env::temp_dir().join(format!("wafer-old-users-blocked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("banned-players.json")).unwrap();
        std::fs::write(dir.join("banned-players.txt"), "Notch|2013-04-01 10:00:00 -0500|Console|Forever|Griefing\r\n").unwrap();
        let services = Services::offline();
        let cache = CachedUserNameToIdResolver::load(Some(dir.clone()));
        let converter = OldUsersConverter {
            server_directory: &dir,
            world_directory: None,
            services: &services,
            name_to_id_cache: &cache,
            uses_authentication: false,
            operator_permission_level: 4,
        };
        assert!(!converter.convert_user_banlist().unwrap());
        assert!(!converter.are_old_userlists_removed());
        assert!(dir.join("banned-players.txt").is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lines_split_on_every_line_terminator() {
        assert_eq!(lines_of(b"a\r\nb\rc\n\nd"), ["a", "b", "c", "", "d"]);
        assert_eq!(lines_of(b"a\n"), ["a"]);
        assert_eq!(lines_of(b""), Vec::<String>::new());
    }
}
