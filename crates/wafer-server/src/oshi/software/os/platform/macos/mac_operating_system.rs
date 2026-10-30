use std::sync::OnceLock;

use super::mac_os_process;
use crate::oshi::OshiResult;
use crate::oshi::software::os::{OSProcess, OSVersionInfo, OperatingSystem};
use crate::oshi::util::executing_command::get_first_answer;
use crate::oshi::util::file_util::parse_properties;
use crate::oshi::util::parse_util::{get_first_int_value, get_nth_int_value, java_trim};
use crate::oshi::util::platform::macos::sysctl_util::sysctl_string;
use crate::util::java_runtime;

const MACOS_VERSIONS_PROPERTIES: &str = include_str!("../../../../oshi.macos.versions.properties");

pub struct MacOperatingSystem {
    os_x_version: String,
    major: i32,
    minor: i32,
    family_version_info: OnceLock<(String, OSVersionInfo)>,
}

impl MacOperatingSystem {
    pub fn new() -> MacOperatingSystem {
        let mut version = java_runtime::os_version();
        let mut major = get_first_int_value(&version);
        let mut minor = get_nth_int_value(&version, 2);
        if major == 10 && minor > 15 {
            let sw_vers = get_first_answer("sw_vers -productVersion");
            if !sw_vers.is_empty() {
                version = sw_vers;
            }
            major = get_first_int_value(&version);
            minor = get_nth_int_value(&version, 2);
        }
        let operating_system = MacOperatingSystem { os_x_version: version, major, minor, family_version_info: OnceLock::new() };
        operating_system.family_version_info();
        operating_system
    }

    fn family_version_info(&self) -> &(String, OSVersionInfo) {
        self.family_version_info.get_or_init(|| {
            let family = if self.major <= 10 && (self.major != 10 || self.minor < 12) { java_runtime::os_name() } else { "macOS".to_string() };
            let code_name = self.parse_code_name();
            let build_number = sysctl_string("kern.osversion", "");
            (family, OSVersionInfo::new(Some(self.os_x_version.clone()), code_name, Some(build_number)))
        })
    }

    fn parse_code_name(&self) -> Option<String> {
        let properties = parse_properties(MACOS_VERSIONS_PROPERTIES);
        let code_name = if self.major > 10 {
            properties.get(&self.major.to_string()).cloned()
        } else if self.major == 10 {
            properties.get(&format!("{}.{}", self.major, self.minor)).cloned()
        } else {
            None
        };
        if code_name.as_deref().is_none_or(|code_name| code_name.is_empty()) {
            warn!("Unable to parse version {}.{} to a codename.", self.major, self.minor);
        }
        code_name
    }
}

impl OperatingSystem for MacOperatingSystem {
    fn to_string(&self) -> OshiResult<String> {
        let (family, version_info) = self.family_version_info();
        Ok(format!("Apple {} {}", java_trim(family), version_info))
    }

    fn is_elevated(&self) -> OshiResult<bool> {
        Ok(get_first_answer("id -u").parse::<i32>().unwrap_or(-1) == 0)
    }

    fn get_process_id(&self) -> i32 {
        std::process::id() as i32
    }

    fn get_process(&self, pid: i32) -> OshiResult<Option<OSProcess>> {
        Ok(mac_os_process::create(pid))
    }
}
