pub const JAVA_VENDOR: &str = "Microsoft";
pub const JAVA_VM_NAME: &str = "OpenJDK 64-Bit Server VM";
pub const JAVA_VM_INFO: &str = "mixed mode, sharing";
pub const JAVA_VM_VENDOR: &str = "Microsoft";

pub fn java_version() -> &'static str {
    if cfg!(all(windows, target_arch = "aarch64")) { "25.0.0" } else { "25.0.1" }
}

pub fn os_arch() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "x86_64") => "x86_64",
        (_, "x86_64") => "amd64",
        ("windows", "x86") => "x86",
        (_, "x86") => "i386",
        (_, other) => other,
    }
}

pub fn input_arguments() -> Vec<String> {
    Vec::new()
}

pub fn available_processors() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[cfg(windows)]
pub fn windows_os_name(major: u32, minor: u32, build: u32, workstation: bool) -> String {
    match (major, minor, workstation) {
        (10, 0, true) if build >= 22000 => "Windows 11".to_string(),
        (10, 0, true) => "Windows 10".to_string(),
        (10, 0, false) if build > 26039 => "Windows Server 2025".to_string(),
        (10, 0, false) if build > 20347 => "Windows Server 2022".to_string(),
        (10, 0, false) if build > 17762 => "Windows Server 2019".to_string(),
        (10, 0, false) => "Windows Server 2016".to_string(),
        (6, 3, true) => "Windows 8.1".to_string(),
        (6, 3, false) => "Windows Server 2012 R2".to_string(),
        (6, 2, true) => "Windows 8".to_string(),
        (6, 2, false) => "Windows Server 2012".to_string(),
        (6, 1, true) => "Windows 7".to_string(),
        (6, 1, false) => "Windows Server 2008 R2".to_string(),
        (6, 0, true) => "Windows Vista".to_string(),
        (6, 0, false) => "Windows Server 2008".to_string(),
        _ => "Windows NT (unknown)".to_string(),
    }
}

#[cfg(windows)]
pub fn windows_version() -> (u32, u32, u32, bool) {
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::SystemInformation::{OSVERSIONINFOEXW, OSVERSIONINFOW};
    let mut info: OSVERSIONINFOEXW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOEXW>() as u32;
    unsafe { RtlGetVersion((&mut info as *mut OSVERSIONINFOEXW).cast::<OSVERSIONINFOW>()) };
    (info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber, info.wProductType == 1)
}

#[cfg(windows)]
pub fn os_name() -> String {
    let (major, minor, build, workstation) = windows_version();
    windows_os_name(major, minor, build, workstation)
}

#[cfg(windows)]
pub fn os_version() -> String {
    let (major, minor, _, _) = windows_version();
    format!("{major}.{minor}")
}

#[cfg(windows)]
pub fn tmpdir() -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetTempPathW;
    let mut buffer = [0u16; 261];
    let length = unsafe { GetTempPathW(buffer.len() as u32, buffer.as_mut_ptr()) } as usize;
    (length > 0 && length < buffer.len()).then(|| String::from_utf16_lossy(&buffer[..length]))
}

#[cfg(target_os = "linux")]
pub fn os_name() -> String {
    "Linux".to_string()
}

#[cfg(target_os = "macos")]
pub fn os_name() -> String {
    "Mac OS X".to_string()
}

#[cfg(unix)]
pub fn os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        let mut buffer = [0u8; 64];
        let mut size = buffer.len();
        let found = unsafe { libc::sysctlbyname(c"kern.osproductversion".as_ptr(), buffer.as_mut_ptr().cast(), &mut size, std::ptr::null_mut(), 0) } == 0;
        if found {
            return String::from_utf8_lossy(&buffer[..size.saturating_sub(1)]).into_owned();
        }
    }
    let mut name: libc::utsname = unsafe { std::mem::zeroed() };
    if unsafe { libc::uname(&mut name) } != 0 {
        return String::new();
    }
    let release = unsafe { std::ffi::CStr::from_ptr(name.release.as_ptr()) };
    release.to_string_lossy().into_owned()
}

#[cfg(unix)]
pub fn tmpdir() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let mut buffer = [0 as libc::c_char; 1024];
        let length = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buffer.as_mut_ptr(), buffer.len()) };
        if length > 0 && length <= buffer.len() {
            return Some(unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }.to_string_lossy().into_owned());
        }
    }
    Some("/tmp".to_string())
}

#[cfg(not(any(windows, unix)))]
pub fn os_name() -> String {
    std::env::consts::OS.to_string()
}

#[cfg(not(any(windows, unix)))]
pub fn os_version() -> String {
    String::new()
}

#[cfg(not(any(windows, unix)))]
pub fn tmpdir() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_names_follow_the_jdk() {
        assert_eq!(windows_os_name(10, 0, 26300, true), "Windows 11");
        assert_eq!(windows_os_name(10, 0, 19045, true), "Windows 10");
        assert_eq!(windows_os_name(10, 0, 26100, false), "Windows Server 2025");
        assert_eq!(windows_os_name(10, 0, 20348, false), "Windows Server 2022");
        assert_eq!(windows_os_name(10, 0, 17763, false), "Windows Server 2019");
        assert_eq!(windows_os_name(10, 0, 14393, false), "Windows Server 2016");
        assert_eq!(windows_os_name(6, 3, 9600, true), "Windows 8.1");
    }

    #[test]
    fn architectures_use_the_java_names() {
        assert!(matches!(os_arch(), "amd64" | "aarch64" | "x86" | "i386" | "x86_64"));
    }
}
