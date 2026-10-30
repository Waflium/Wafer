use std::sync::OnceLock;

use super::windows_os_process;
use crate::jna::platform::win32::advapi32_util::is_current_process_elevated;
use crate::oshi::OshiResult;
use crate::oshi::driver::windows::registry::process_performance_data::{build_process_map_from_perf_counters, build_process_map_from_registry};
use crate::oshi::driver::windows::registry::process_wts_data::query_process_wts_map;
use crate::oshi::driver::windows::wmi::win32_operating_system;
use crate::oshi::software::os::{OSProcess, OSVersionInfo, OperatingSystem};
use crate::util::{java_runtime, java_stack};

const GET_PROCESS_FRAME: &str = "oshi.software.os.windows.WindowsOperatingSystem.getProcess(WindowsOperatingSystem.java:292)";
const PROCESS_MAP_TO_LIST_FRAME: &str = "oshi.software.os.windows.WindowsOperatingSystem.processMapToList(WindowsOperatingSystem.java:298)";
const MEMOIZER_GET_FRAME: &str = "oshi.util.Memoizer$1.get(Memoizer.java:65)";
const QUERY_PROCESS_MAP_FROM_REGISTRY_FRAME: &str = "oshi.software.os.windows.WindowsOperatingSystem.queryProcessMapFromRegistry(WindowsOperatingSystem.java:328)";

#[derive(Default)]
pub struct WindowsOperatingSystem {
    family_version_info: OnceLock<(String, OSVersionInfo)>,
}

impl WindowsOperatingSystem {
    pub fn new() -> WindowsOperatingSystem {
        WindowsOperatingSystem::default()
    }

    fn query_manufacturer(&self) -> &'static str {
        "Microsoft"
    }

    fn family_version_info(&self) -> &(String, OSVersionInfo) {
        self.family_version_info.get_or_init(query_family_version_info)
    }
}

fn parse_code_name(suite_mask: i32) -> String {
    let mut suites = Vec::new();
    for (mask, name) in [
        (2, "Enterprise"),
        (4, "BackOffice"),
        (8, "Communications Server"),
        (128, "Datacenter"),
        (512, "Home"),
        (1024, "Web Server"),
        (8192, "Storage Server"),
        (16384, "Compute Cluster"),
        (32768, "Home Server"),
    ] {
        if suite_mask & mask != 0 {
            suites.push(name);
        }
    }
    suites.join(",")
}

pub fn windows_version_name(os_name: &str, sp: Option<&str>, build_number: &str) -> String {
    let mut version = os_name.strip_prefix("Windows ").unwrap_or(os_name).to_string();
    if let Some(sp) = sp.filter(|sp| !sp.is_empty() && *sp != "unknown") {
        version = format!("{version} {}", sp.replace("Service Pack ", "SP"));
    }
    if version == "10" && build_number >= "22000" {
        version = "11".to_string();
    }
    if version == "Server 2016" && build_number > "17762" {
        version = "Server 2019".to_string();
    }
    if version == "Server 2019" && build_number > "20347" {
        version = "Server 2022".to_string();
    }
    if version == "Server 2022" && build_number > "26039" {
        version = "Server 2025".to_string();
    }
    version
}

fn query_family_version_info() -> (String, OSVersionInfo) {
    let version_info = win32_operating_system::query_os_version();
    let (mut sp, mut suite_mask, mut build_number) = (None, 0, String::new());
    if version_info.get_result_count() > 0 {
        sp = Some(version_info.get_string(0, win32_operating_system::CSDVERSION));
        suite_mask = version_info.get_int(0, win32_operating_system::SUITEMASK);
        build_number = version_info.get_string(0, win32_operating_system::BUILDNUMBER);
    }
    let code_name = parse_code_name(suite_mask);
    let version = windows_version_name(&java_runtime::os_name(), sp.as_deref(), &build_number);
    ("Windows".to_string(), OSVersionInfo::new(Some(version), Some(code_name), Some(build_number)))
}

impl OperatingSystem for WindowsOperatingSystem {
    fn to_string(&self) -> OshiResult<String> {
        let (family, version_info) = self.family_version_info();
        Ok(format!("{} {} {}", self.query_manufacturer(), family, version_info))
    }

    fn is_elevated(&self) -> OshiResult<bool> {
        Ok(is_current_process_elevated())
    }

    fn get_process_id(&self) -> i32 {
        std::process::id() as i32
    }

    fn get_process(&self, pid: i32) -> OshiResult<Option<OSProcess>> {
        let _get_process = java_stack::enter(GET_PROCESS_FRAME);
        let pids = [pid];
        let process_map_to_list = java_stack::enter(PROCESS_MAP_TO_LIST_FRAME);
        let mut process_map = {
            let _memoizer = java_stack::enter(MEMOIZER_GET_FRAME);
            let _query = java_stack::enter(QUERY_PROCESS_MAP_FROM_REGISTRY_FRAME);
            build_process_map_from_registry(None)
        };
        if process_map.as_ref().is_none_or(|map| map.is_empty()) {
            process_map = Some(build_process_map_from_perf_counters(Some(&pids))?);
        }
        let process_map = process_map.unwrap_or_default();
        drop(process_map_to_list);
        let process_wts_map = query_process_wts_map(Some(&pids));
        Ok(windows_os_process::create(pid, &process_map, &process_wts_map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suites_are_listed_in_order() {
        assert_eq!(parse_code_name(272), "");
        assert_eq!(parse_code_name(2 | 128 | 512), "Enterprise,Datacenter,Home");
    }

    #[test]
    fn version_names_follow_the_build_number() {
        assert_eq!(windows_version_name("Windows 11", None, "26300"), "11");
        assert_eq!(windows_version_name("Windows 10", Some(""), "22631"), "11");
        assert_eq!(windows_version_name("Windows 10", Some("unknown"), "19045"), "10");
        assert_eq!(windows_version_name("Windows 7", Some("Service Pack 1"), "7601"), "7 SP1");
        assert_eq!(windows_version_name("Windows Server 2016", None, "20348"), "Server 2022");
        assert_eq!(windows_version_name("Windows Server 2016", None, "26100"), "Server 2025");
    }

    #[test]
    fn this_machine_describes_its_operating_system() {
        let description = WindowsOperatingSystem::new().to_string().ok().unwrap();
        assert!(description.starts_with("Microsoft Windows "));
        assert!(description.contains(" build "));
    }
}
