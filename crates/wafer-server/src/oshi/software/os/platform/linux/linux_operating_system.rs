use std::sync::OnceLock;

use super::linux_os_process;
use crate::oshi::OshiResult;
use crate::oshi::software::os::{OSProcess, OSVersionInfo, OperatingSystem};
use crate::oshi::util::executing_command::{get_first_answer, run_native};
use crate::oshi::util::file_util::{get_string_from_file, parse_properties, read_file};
use crate::oshi::util::parse_util::{java_trim, parse_long_or_default, split_whitespaces};

const OSHI_LINUX_FILENAME_PROPERTIES: &str = include_str!("../../../../oshi.linux.filename.properties");

pub struct LinuxStatics {
    pub user_hz: i64,
    pub page_size: i64,
    pub os_name: String,
    pub boot_time: i64,
}

fn check_udev() {
    let found = ["libudev.so", "libudev.so.1", "libudev.so.0"].iter().any(|library| {
        let name = std::ffi::CString::new(*library).unwrap_or_default();
        let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_LAZY) };
        let loaded = !handle.is_null();
        if loaded {
            unsafe { libc::dlclose(handle) };
        }
        loaded
    });
    if !found {
        warn!("Did not find udev library in operating system. Some features may not work.");
    }
}

fn query_boot_time() -> i64 {
    for stat in read_file("/proc/stat") {
        if stat.starts_with("btime") {
            let boot_time = split_whitespaces(&stat);
            return parse_long_or_default(boot_time.get(1).copied().unwrap_or(""), 0);
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() as i64);
    let uptime = get_string_from_file("/proc/uptime");
    now - uptime.split(' ').next().and_then(|seconds| seconds.parse::<f64>().ok()).unwrap_or(0.0) as i64
}

pub fn statics() -> &'static LinuxStatics {
    static STATICS: OnceLock<LinuxStatics> = OnceLock::new();
    STATICS.get_or_init(|| {
        check_udev();
        let hz = unsafe { libc::getauxval(libc::AT_CLKTCK) } as i64;
        let user_hz = if hz > 0 { hz } else { parse_long_or_default(&get_first_answer("getconf CLK_TCK"), 100) };
        let page_size = unsafe { libc::getauxval(libc::AT_PAGESZ) } as i64;
        let page_size = if page_size > 0 { page_size } else { parse_long_or_default(&get_first_answer("getconf PAGE_SIZE"), 4096) };
        let os_name = get_first_answer("uname -o");
        LinuxStatics { user_hz, page_size, os_name, boot_time: query_boot_time() }
    })
}

#[derive(Default)]
pub struct LinuxOperatingSystem {
    family_version_info: OnceLock<(String, OSVersionInfo)>,
}

impl LinuxOperatingSystem {
    pub fn new() -> LinuxOperatingSystem {
        let operating_system = LinuxOperatingSystem::default();
        operating_system.family_version_info();
        operating_system
    }

    fn family_version_info(&self) -> &(String, OSVersionInfo) {
        self.family_version_info.get_or_init(query_family_version_info)
    }
}

type FamilyVersionCodename = (String, String, String);

fn unquote(line: &str) -> String {
    let line = line.strip_prefix('"').unwrap_or(line);
    let line = line.strip_suffix('"').unwrap_or(line);
    java_trim(line).to_string()
}

fn split_keeping_java_semantics<'a>(line: &'a str, separators: &[char]) -> Vec<&'a str> {
    let mut parts: Vec<&str> = line.split(|character| separators.contains(&character)).collect();
    while parts.len() > 1 && parts.last().is_some_and(|last| last.is_empty()) {
        parts.pop();
    }
    if parts.len() == 1 && parts[0].is_empty() && !line.is_empty() {
        parts.pop();
    }
    parts
}

fn query_family_version_info() -> (String, OSVersionInfo) {
    let (family, version_id, code_name) = query_family_version_codename_from_release_files();
    let mut build_number = None;
    let proc_version = read_file("/proc/version");
    if let Some(first) = proc_version.first() {
        for token in split_whitespaces(first) {
            if token != "Linux" && token != "version" {
                build_number = Some(token.to_string());
                break;
            }
        }
    }
    (family, OSVersionInfo::new(Some(version_id), Some(code_name), build_number))
}

fn query_family_version_codename_from_release_files() -> FamilyVersionCodename {
    if let Some(result) = read_distrib_release("/etc/system-release") {
        return result;
    }
    if let Some(result) = read_os_release() {
        return result;
    }
    if let Some(result) = exec_lsb_release() {
        return result;
    }
    if let Some(result) = read_lsb_release() {
        return result;
    }
    let etc_distrib_release = get_release_filename();
    if let Some(result) = read_distrib_release(&etc_distrib_release) {
        return result;
    }
    let name = etc_distrib_release.replace("/etc/", "").replace("release", "").replace("version", "").replace('-', "").replace('_', "");
    (filename_to_family(&name), "unknown".to_string(), "unknown".to_string())
}

fn read_os_release() -> Option<FamilyVersionCodename> {
    let mut family: Option<String> = None;
    let mut version_id = "unknown".to_string();
    let mut code_name = "unknown".to_string();
    for line in read_file("/etc/os-release") {
        if let Some(version) = line.strip_prefix("VERSION=") {
            let line = unquote(version);
            let mut split = split_keeping_java_semantics(&line, &['(', ')']);
            if split.len() <= 1 {
                split = line.split(", ").collect();
            }
            if !split.is_empty() {
                version_id = java_trim(split[0]).to_string();
            }
            if split.len() > 1 {
                code_name = java_trim(split[1]).to_string();
            }
        } else if let Some(name) = line.strip_prefix("NAME=").filter(|_| family.is_none()) {
            family = Some(unquote(name));
        } else if let Some(id) = line.strip_prefix("VERSION_ID=").filter(|_| version_id == "unknown") {
            version_id = unquote(id);
        }
    }
    family.map(|family| (family, version_id, code_name))
}

fn exec_lsb_release() -> Option<FamilyVersionCodename> {
    let mut family: Option<String> = None;
    let mut version_id = "unknown".to_string();
    let mut code_name = "unknown".to_string();
    for line in run_native("lsb_release -a") {
        if let Some(description) = line.strip_prefix("Description:") {
            let description = java_trim(description);
            if description.contains(" release ") {
                let (triplet_family, triplet_version, triplet_code_name) = parse_release(description, " release ");
                family = Some(triplet_family);
                if version_id == "unknown" {
                    version_id = triplet_version;
                }
                if code_name == "unknown" {
                    code_name = triplet_code_name;
                }
            }
        } else if let Some(id) = line.strip_prefix("Distributor ID:").filter(|_| family.is_none()) {
            family = Some(java_trim(id).to_string());
        } else if let Some(release) = line.strip_prefix("Release:").filter(|_| version_id == "unknown") {
            version_id = java_trim(release).to_string();
        } else if let Some(codename) = line.strip_prefix("Codename:").filter(|_| code_name == "unknown") {
            code_name = java_trim(codename).to_string();
        }
    }
    family.map(|family| (family, version_id, code_name))
}

fn read_lsb_release() -> Option<FamilyVersionCodename> {
    let mut family: Option<String> = None;
    let mut version_id = "unknown".to_string();
    let mut code_name = "unknown".to_string();
    for line in read_file("/etc/lsb-release") {
        if let Some(description) = line.strip_prefix("DISTRIB_DESCRIPTION=") {
            let description = unquote(description);
            if description.contains(" release ") {
                let (triplet_family, triplet_version, triplet_code_name) = parse_release(&description, " release ");
                family = Some(triplet_family);
                if version_id == "unknown" {
                    version_id = triplet_version;
                }
                if code_name == "unknown" {
                    code_name = triplet_code_name;
                }
            }
        } else if let Some(id) = line.strip_prefix("DISTRIB_ID=").filter(|_| family.is_none()) {
            family = Some(unquote(id));
        } else if let Some(release) = line.strip_prefix("DISTRIB_RELEASE=").filter(|_| version_id == "unknown") {
            version_id = unquote(release);
        } else if let Some(codename) = line.strip_prefix("DISTRIB_CODENAME=").filter(|_| code_name == "unknown") {
            code_name = unquote(codename);
        }
    }
    family.map(|family| (family, version_id, code_name))
}

fn read_distrib_release(filename: &str) -> Option<FamilyVersionCodename> {
    if std::path::Path::new(filename).exists() {
        for line in read_file(filename) {
            if line.contains(" release ") {
                return Some(parse_release(&line, " release "));
            }
            if line.contains(" VERSION ") {
                return Some(parse_release(&line, " VERSION "));
            }
        }
    }
    None
}

fn parse_release(line: &str, split_line: &str) -> FamilyVersionCodename {
    let mut split: Vec<&str> = line.split(split_line).collect();
    while split.len() > 1 && split.last().is_some_and(|last| last.is_empty()) {
        split.pop();
    }
    let family = java_trim(split[0]).to_string();
    let mut version_id = "unknown".to_string();
    let mut code_name = "unknown".to_string();
    if split.len() > 1 {
        let parts = split_keeping_java_semantics(split[1], &['(', ')']);
        if !parts.is_empty() {
            version_id = java_trim(parts[0]).to_string();
        }
        if parts.len() > 1 {
            code_name = java_trim(parts[1]).to_string();
        }
    }
    (family, version_id, code_name)
}

fn get_release_filename() -> String {
    let matching = std::fs::read_dir("/etc").ok().and_then(|entries| {
        entries.flatten().map(|entry| entry.path()).find(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            (name.ends_with("-release") || name.ends_with("-version") || name.ends_with("_release") || name.ends_with("_version"))
                && !name.ends_with("os-release")
                && !name.ends_with("lsb-release")
                && !name.ends_with("system-release")
        })
    });
    match matching {
        Some(path) => path.to_string_lossy().into_owned(),
        None if std::path::Path::new("/etc/release").exists() => "/etc/release".to_string(),
        None => "/etc/issue".to_string(),
    }
}

fn filename_to_family(name: &str) -> String {
    if name.is_empty() {
        return "Solaris".to_string();
    }
    if name.eq_ignore_ascii_case("issue") {
        return "Unknown".to_string();
    }
    match parse_properties(OSHI_LINUX_FILENAME_PROPERTIES).get(&name.to_lowercase()) {
        Some(family) => family.clone(),
        None => {
            let mut characters = name.chars();
            characters.next().map_or_else(String::new, |first| first.to_uppercase().collect::<String>() + characters.as_str())
        }
    }
}

impl OperatingSystem for LinuxOperatingSystem {
    fn to_string(&self) -> OshiResult<String> {
        let (family, version_info) = self.family_version_info();
        Ok(format!("{} {} {}", statics().os_name, family, version_info))
    }

    fn is_elevated(&self) -> OshiResult<bool> {
        Ok(get_first_answer("id -u").parse::<i32>().unwrap_or(-1) == 0)
    }

    fn get_process_id(&self) -> i32 {
        std::process::id() as i32
    }

    fn get_process(&self, pid: i32) -> OshiResult<Option<OSProcess>> {
        Ok(linux_os_process::create(pid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_split_into_family_version_and_code_name() {
        assert_eq!(parse_release("CentOS Linux release 7.9.2009 (Core)", " release "), ("CentOS Linux".into(), "7.9.2009".into(), "Core".into()));
        assert_eq!(parse_release("Fedora release 39", " release "), ("Fedora".into(), "39".into(), "unknown".into()));
    }

    #[test]
    fn quotes_are_removed_from_release_values() {
        assert_eq!(unquote("\"24.04.1 LTS (Noble Numbat)\""), "24.04.1 LTS (Noble Numbat)");
        assert_eq!(unquote("Ubuntu"), "Ubuntu");
    }

    #[test]
    fn file_names_map_to_families() {
        assert_eq!(filename_to_family(""), "Solaris");
        assert_eq!(filename_to_family("issue"), "Unknown");
        assert_eq!(filename_to_family("redhat"), "Red Hat Linux");
        assert_eq!(filename_to_family("zzzdistro"), "Zzzdistro");
    }

    #[test]
    fn this_machine_describes_its_operating_system() {
        assert!(!LinuxOperatingSystem::new().to_string().ok().unwrap().is_empty());
    }
}
