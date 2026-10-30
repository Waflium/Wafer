pub struct NativeModuleVersion {
    pub description: String,
    pub version: String,
    pub company: String,
}

pub struct NativeModuleInfo {
    pub name: String,
    pub version: Option<NativeModuleVersion>,
}

impl std::fmt::Display for NativeModuleInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.version {
            Some(version) => write!(formatter, "{}:{}:{}:{}", self.name, version.description, version.version, version.company),
            None => write!(formatter, "{}", self.name),
        }
    }
}

#[cfg(windows)]
pub fn list_modules() -> Vec<NativeModuleInfo> {
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::ProcessStatus::{EnumProcessModules, GetModuleFileNameExW};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let process = unsafe { GetCurrentProcess() };
    let mut modules: Vec<HMODULE> = vec![std::ptr::null_mut(); 1024];
    let mut needed = 0u32;
    let capacity_bytes = (modules.len() * std::mem::size_of::<HMODULE>()) as u32;
    if unsafe { EnumProcessModules(process, modules.as_mut_ptr(), capacity_bytes, &mut needed) } == 0 {
        return Vec::new();
    }
    modules.truncate(needed as usize / std::mem::size_of::<HMODULE>());
    let mut result = Vec::new();
    for module in modules {
        let mut path = vec![0u16; 32768];
        let length = unsafe { GetModuleFileNameExW(process, module, path.as_mut_ptr(), path.len() as u32) } as usize;
        if length == 0 {
            continue;
        }
        let path = String::from_utf16_lossy(&path[..length]);
        let name = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
        result.push(NativeModuleInfo { name, version: try_get_module_version(&path) });
    }
    result
}

#[cfg(not(windows))]
pub fn list_modules() -> Vec<NativeModuleInfo> {
    Vec::new()
}

#[cfg(windows)]
const DEFAULT_LANG: u32 = 1033;
#[cfg(windows)]
const DEFAULT_CODEPAGE: u32 = 78643200;

#[cfg(windows)]
fn try_get_module_version(path: &str) -> Option<NativeModuleVersion> {
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW};

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut dummy = 0u32;
    let version_length = unsafe { GetFileVersionInfoSizeW(wide.as_ptr(), &mut dummy) };
    if version_length == 0 {
        return None;
    }
    let mut data = vec![0u8; version_length as usize];
    if unsafe { GetFileVersionInfoW(wide.as_ptr(), 0, version_length, data.as_mut_ptr().cast()) } == 0 {
        return None;
    }
    let translations = query_version_value(&data, "\\VarFileInfo\\Translation")?;
    let lang_and_codepage = find_lang_and_codepage(&translations)?;
    let lang = lang_and_codepage & 65535;
    let codepage = (lang_and_codepage & 0xFFFF_0000) >> 16;
    let query_string = |key: &str| query_version_string(&data, &format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\{key}"));
    Some(NativeModuleVersion { description: query_string("FileDescription"), version: query_string("FileVersion"), company: query_string("CompanyName") })
}

#[cfg(windows)]
fn find_lang_and_codepage(translations: &[u8]) -> Option<u32> {
    let mut best_so_far = None;
    for chunk in translations.chunks_exact(4) {
        let lang_and_codepage = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if lang_and_codepage & 0xFFFF_0000 == DEFAULT_CODEPAGE && lang_and_codepage & 65535 == DEFAULT_LANG {
            return Some(lang_and_codepage);
        }
        best_so_far = Some(lang_and_codepage);
    }
    best_so_far
}

#[cfg(windows)]
fn query_version_value(data: &[u8], key: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;

    let wide_key: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buffer: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut size = 0u32;
    if unsafe { VerQueryValueW(data.as_ptr().cast(), wide_key.as_ptr(), &mut buffer, &mut size) } == 0 || buffer.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), size as usize) }.to_vec())
}

#[cfg(windows)]
fn query_version_string(data: &[u8], key: &str) -> String {
    use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;

    let wide_key: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buffer: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut size = 0u32;
    if unsafe { VerQueryValueW(data.as_ptr().cast(), wide_key.as_ptr(), &mut buffer, &mut size) } == 0 || buffer.is_null() || size == 0 {
        return String::new();
    }
    let units = unsafe { std::slice::from_raw_parts(buffer.cast::<u16>(), size as usize - 1) };
    String::from_utf16_lossy(units)
}
