use crate::oshi::util::platform::windows::wmi_query_handler::{WmiResult, query_wmi};

pub mod win32_physical_memory {
    use super::*;

    pub const BANKLABEL: usize = 0;
    pub const CAPACITY: usize = 1;
    pub const SPEED: usize = 2;
    pub const MANUFACTURER: usize = 3;
    pub const PARTNUMBER: usize = 4;
    pub const SMBIOSMEMORYTYPE: usize = 5;
    pub const SERIALNUMBER: usize = 6;

    pub fn queryphysical_memory() -> WmiResult {
        query_wmi("Win32_PhysicalMemory", &["BANKLABEL", "CAPACITY", "SPEED", "MANUFACTURER", "PARTNUMBER", "SMBIOSMEMORYTYPE", "SERIALNUMBER"])
    }
}

pub mod win32_operating_system {
    use super::*;

    pub const BUILDNUMBER: usize = 0;
    pub const CSDVERSION: usize = 1;
    pub const SUITEMASK: usize = 2;

    pub fn query_os_version() -> WmiResult {
        query_wmi("Win32_OperatingSystem", &["BUILDNUMBER", "CSDVERSION", "SUITEMASK"])
    }
}

pub mod win32_video_controller {
    use super::*;

    pub const ADAPTERCOMPATIBILITY: usize = 0;
    pub const ADAPTERRAM: usize = 1;
    pub const DRIVERVERSION: usize = 2;
    pub const NAME: usize = 3;
    pub const PNPDEVICEID: usize = 4;

    pub fn query_video_controller() -> WmiResult {
        query_wmi("Win32_VideoController", &["ADAPTERCOMPATIBILITY", "ADAPTERRAM", "DRIVERVERSION", "NAME", "PNPDEVICEID"])
    }
}
