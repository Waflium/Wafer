pub mod platform;

use super::super::OshiResult;
use crate::util::java_stack;

const GET_CURRENT_PROCESS_FRAME: &str = "oshi.software.os.OperatingSystem.getCurrentProcess(OperatingSystem.java:246)";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OSVersionInfo {
    version: Option<String>,
    code_name: Option<String>,
    build_number: Option<String>,
    version_str: String,
}

impl OSVersionInfo {
    pub fn new(version: Option<String>, code_name: Option<String>, build_number: Option<String>) -> OSVersionInfo {
        let mut version_str = version.clone().unwrap_or_else(|| "unknown".to_string());
        if let Some(code_name) = code_name.as_deref().filter(|code_name| !code_name.is_empty()) {
            version_str.push_str(" (");
            version_str.push_str(code_name);
            version_str.push(')');
        }
        if let Some(build_number) = build_number.as_deref().filter(|build_number| !build_number.is_empty()) {
            version_str.push_str(" build ");
            version_str.push_str(build_number);
        }
        OSVersionInfo { version, code_name, build_number, version_str }
    }
}

impl std::fmt::Display for OSVersionInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.version_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OSProcess {
    pub virtual_size: i64,
    pub resident_set_size: i64,
    pub kernel_time: i64,
    pub user_time: i64,
    pub up_time: i64,
}

impl OSProcess {
    pub fn get_virtual_size(&self) -> i64 {
        self.virtual_size
    }

    pub fn get_resident_set_size(&self) -> i64 {
        self.resident_set_size
    }

    pub fn get_kernel_time(&self) -> i64 {
        self.kernel_time
    }

    pub fn get_user_time(&self) -> i64 {
        self.user_time
    }

    pub fn get_up_time(&self) -> i64 {
        self.up_time
    }
}

pub trait OperatingSystem {
    fn to_string(&self) -> OshiResult<String>;

    fn is_elevated(&self) -> OshiResult<bool>;

    fn get_process_id(&self) -> i32;

    fn get_process(&self, pid: i32) -> OshiResult<Option<OSProcess>>;

    fn get_current_process(&self) -> OshiResult<Option<OSProcess>> {
        let _scope = java_stack::enter(GET_CURRENT_PROCESS_FRAME);
        self.get_process(self.get_process_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_info_formats_code_name_and_build() {
        assert_eq!(OSVersionInfo::new(Some("11".into()), Some(String::new()), Some("26300".into())).to_string(), "11 build 26300");
        assert_eq!(OSVersionInfo::new(Some("10".into()), Some("Enterprise,Home".into()), Some("19045".into())).to_string(), "10 (Enterprise,Home) build 19045");
        assert_eq!(OSVersionInfo::new(None, None, None).to_string(), "unknown");
        assert_eq!(OSVersionInfo::new(Some("22.04.3 LTS".into()), Some("Jammy Jellyfish".into()), Some("5.15.0".into())).to_string(), "22.04.3 LTS (Jammy Jellyfish) build 5.15.0");
    }
}
