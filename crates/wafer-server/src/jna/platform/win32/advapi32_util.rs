use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, RRF_RT_ANY, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, RegQueryInfoKeyW, RegQueryValueExW,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::win32_exception::Win32Exception;
use super::{from_wide_nul, wide};
use crate::util::java_stack;
use crate::util::java_throwable::JavaThrowable;

pub const ROOT_LOCAL_MACHINE: HKEY = HKEY_LOCAL_MACHINE;

const KEY_READ: u32 = 0x20019;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const REG_SZ: u32 = 1;
const REG_BINARY: u32 = 3;
const REG_DWORD: u32 = 4;
const REG_MULTI_SZ: u32 = 7;
const REG_QWORD: u32 = 11;

const ADVAPI32_UTIL: &str = "com.sun.jna.platform.win32.Advapi32Util";

fn advapi32_frame(method: &str, line: u32) -> &'static str {
    java_stack::frame(&format!("{ADVAPI32_UTIL}.{method}"), "Advapi32Util.java", line)
}

pub enum RegistryError {
    Win32(Win32Exception),
    Other(JavaThrowable),
}

impl From<Win32Exception> for RegistryError {
    fn from(exception: Win32Exception) -> RegistryError {
        RegistryError::Win32(exception)
    }
}

impl From<RegistryError> for JavaThrowable {
    fn from(error: RegistryError) -> JavaThrowable {
        match error {
            RegistryError::Win32(exception) => exception.throwable,
            RegistryError::Other(throwable) => throwable,
        }
    }
}

pub enum RegistryValue {
    None,
    Int(i32),
    Long(i64),
    Binary(Vec<u8>),
}

struct OpenKey(HKEY);

impl Drop for OpenKey {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn open_key(root: HKEY, path: &str, open_line: u32, entry_frames: &[(&str, u32)]) -> Result<OpenKey, Win32Exception> {
    let mut key: HKEY = std::ptr::null_mut();
    let result = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) };
    if result != 0 {
        let frames: Vec<&'static str> = std::iter::once(advapi32_frame(entry_frames[0].0, open_line)).chain(entry_frames.iter().skip(1).map(|(method, line)| advapi32_frame(method, *line))).collect();
        return Err(Win32Exception::new(result as i32, &frames));
    }
    Ok(OpenKey(key))
}

fn runtime_exception(message: String) -> RegistryError {
    RegistryError::Other(JavaThrowable::new(format!("java.lang.RuntimeException: {message}"), java_stack::capture(&[])))
}

pub fn registry_get_keys(root: HKEY, path: &str) -> Result<Vec<String>, RegistryError> {
    let key = open_key(root, path, 2043, &[("registryGetKeys", 0), ("registryGetKeys", 2023)])?;
    let (mut sub_keys, mut max_sub_key_length) = (0u32, 0u32);
    let result = unsafe {
        RegQueryInfoKeyW(
            key.0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut sub_keys,
            &mut max_sub_key_length,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result != 0 {
        return Err(Win32Exception::new(result as i32, &[advapi32_frame("registryQueryInfoKey", 0)]).into());
    }
    let mut names = Vec::with_capacity(sub_keys as usize);
    for index in 0..sub_keys {
        let mut name = vec![0u16; max_sub_key_length as usize + 2];
        let mut length = name.len() as u32;
        let result = unsafe {
            RegEnumKeyExW(key.0, index, name.as_mut_ptr(), &mut length, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
        };
        if result != 0 {
            return Err(Win32Exception::new(result as i32, &[advapi32_frame("registryRegEnumKey", 0)]).into());
        }
        names.push(from_wide_nul(&name[..length as usize]));
    }
    Ok(names)
}

fn query_value(key: &OpenKey, value: &str, line: u32, method: &'static str) -> Result<(u32, Vec<u8>), RegistryError> {
    let name = wide(value);
    let (mut value_type, mut size) = (0u32, 0u32);
    let result = unsafe { RegQueryValueExW(key.0, name.as_ptr(), std::ptr::null(), &mut value_type, std::ptr::null_mut(), &mut size) };
    if result != 0 && result != ERROR_INSUFFICIENT_BUFFER {
        return Err(Win32Exception::new(result as i32, &[advapi32_frame(method, line)]).into());
    }
    let mut data = vec![0u8; size as usize + 4];
    let mut capacity = size;
    let result = unsafe { RegQueryValueExW(key.0, name.as_ptr(), std::ptr::null(), &mut value_type, data.as_mut_ptr(), &mut capacity) };
    if result != 0 && result != ERROR_INSUFFICIENT_BUFFER {
        return Err(Win32Exception::new(result as i32, &[advapi32_frame(method, line)]).into());
    }
    data.truncate(capacity as usize);
    Ok((value_type, data))
}

fn wide_units(data: &[u8]) -> Vec<u16> {
    data.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect()
}

pub fn registry_get_string_value(root: HKEY, path: &str, value: &str) -> Result<String, RegistryError> {
    let key = open_key(root, path, 741, &[("registryGetStringValue", 0), ("registryGetStringValue", 718)])?;
    let (value_type, data) = query_value(&key, value, 774, "registryGetStringValue")?;
    if value_type != REG_SZ {
        return Err(runtime_exception(format!("Unexpected registry type {value_type}, expected REG_SZ")));
    }
    Ok(from_wide_nul(&wide_units(&data)))
}

pub fn registry_get_int_value(root: HKEY, path: &str, value: &str) -> Result<i32, RegistryError> {
    let key = open_key(root, path, 1118, &[("registryGetIntValue", 0), ("registryGetIntValue", 1096)])?;
    let (value_type, data) = query_value(&key, value, 1146, "registryGetIntValue")?;
    if value_type != REG_DWORD {
        return Err(runtime_exception(format!("Unexpected registry type {value_type}, expected REG_DWORD")));
    }
    Ok(i32::from_le_bytes(data.get(..4).and_then(|bytes| bytes.try_into().ok()).unwrap_or([0; 4])))
}

pub fn registry_get_long_value(root: HKEY, path: &str, value: &str) -> Result<i64, RegistryError> {
    let key = open_key(root, path, 1196, &[("registryGetLongValue", 0), ("registryGetLongValue", 1174)])?;
    let (value_type, data) = query_value(&key, value, 1224, "registryGetLongValue")?;
    if value_type != REG_QWORD {
        return Err(runtime_exception(format!("Unexpected registry type {value_type}, expected REG_QWORD")));
    }
    Ok(i64::from_le_bytes(data.get(..8).and_then(|bytes| bytes.try_into().ok()).unwrap_or([0; 8])))
}

pub fn registry_value_exists(root: HKEY, path: &str, value: &str) -> Result<bool, RegistryError> {
    let mut key: HKEY = std::ptr::null_mut();
    let result = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) };
    match result {
        0 => {}
        ERROR_FILE_NOT_FOUND => return Ok(false),
        other => return Err(Win32Exception::new(other as i32, &[advapi32_frame("registryValueExists", 689), advapi32_frame("registryValueExists", 650)]).into()),
    }
    let key = OpenKey(key);
    let result = unsafe { RegQueryValueExW(key.0, wide(value).as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
    match result {
        0 => Ok(true),
        ERROR_FILE_NOT_FOUND => Ok(false),
        other => Err(Win32Exception::new(other as i32, &[advapi32_frame("registryValueExists", 698)]).into()),
    }
}

pub fn registry_get_value(root: HKEY, path: &str, value: &str) -> Result<RegistryValue, RegistryError> {
    let (path_wide, value_wide) = (wide(path), wide(value));
    let (mut value_type, mut size) = (0u32, 0u32);
    let result = unsafe { RegGetValueW(root, path_wide.as_ptr(), value_wide.as_ptr(), RRF_RT_ANY, &mut value_type, std::ptr::null_mut(), &mut size) };
    if value_type == 0 {
        return Ok(RegistryValue::None);
    }
    if result != 0 && result != ERROR_INSUFFICIENT_BUFFER {
        return Err(Win32Exception::new(result as i32, &[advapi32_frame("registryGetValue", 1263)]).into());
    }
    let mut data = vec![0u8; size as usize + 2];
    let result = unsafe { RegGetValueW(root, path_wide.as_ptr(), value_wide.as_ptr(), RRF_RT_ANY, &mut value_type, data.as_mut_ptr().cast(), &mut size) };
    if result != 0 {
        return Err(Win32Exception::new(result as i32, &[advapi32_frame("registryGetValue", 1275)]).into());
    }
    let read = |length: usize| -> Vec<u8> { data.iter().copied().take(length).chain(std::iter::repeat(0)).take(length).collect() };
    Ok(match value_type {
        REG_DWORD => RegistryValue::Int(i32::from_le_bytes(read(4).try_into().unwrap_or([0; 4]))),
        REG_QWORD => RegistryValue::Long(i64::from_le_bytes(read(8).try_into().unwrap_or([0; 8]))),
        REG_BINARY => RegistryValue::Binary(data[..(size as usize).min(data.len())].to_vec()),
        _ => RegistryValue::None,
    })
}

pub fn registry_get_string_array(root: HKEY, path: &str, value: &str) -> Result<Vec<String>, RegistryError> {
    let key = open_key(root, path, 921, &[("registryGetStringArray", 0), ("registryGetStringArray", 898)])?;
    let name = wide(value);
    let (mut value_type, mut size) = (0u32, 0u32);
    let result = unsafe { RegQueryValueExW(key.0, name.as_ptr(), std::ptr::null(), &mut value_type, std::ptr::null_mut(), &mut size) };
    if result != 0 && result != ERROR_INSUFFICIENT_BUFFER {
        return Err(Win32Exception::new(
            result as i32,
            &[advapi32_frame("registryGetStringArray", 947), advapi32_frame("registryGetStringArray", 924), advapi32_frame("registryGetStringArray", 898)],
        )
        .into());
    }
    if value_type != REG_MULTI_SZ {
        return Err(runtime_exception(format!("Unexpected registry type {value_type}, expected REG_SZ")));
    }
    let mut data = vec![0u8; size as usize + 4];
    let mut capacity = size;
    let result = unsafe { RegQueryValueExW(key.0, name.as_ptr(), std::ptr::null(), &mut value_type, data.as_mut_ptr(), &mut capacity) };
    if result != 0 && result != ERROR_INSUFFICIENT_BUFFER {
        return Err(Win32Exception::new(
            result as i32,
            &[advapi32_frame("registryGetStringArray", 949), advapi32_frame("registryGetStringArray", 924), advapi32_frame("registryGetStringArray", 898)],
        )
        .into());
    }
    Ok(reg_multi_sz_to_string_array(&wide_units(&data)))
}

pub fn reg_multi_sz_to_string_array(units: &[u16]) -> Vec<String> {
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < units.len() {
        let end = units[offset..].iter().position(|&unit| unit == 0).map_or(units.len(), |position| offset + position);
        if end == offset {
            break;
        }
        result.push(String::from_utf16_lossy(&units[offset..end]));
        offset = end + 1;
    }
    result
}

pub fn is_current_process_elevated() -> bool {
    let mut token: HANDLE = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut length = 0u32;
    let elevated = unsafe {
        GetTokenInformation(token, TokenElevation, (&mut elevation as *mut TOKEN_ELEVATION).cast(), std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut length)
    } != 0
        && elevation.TokenIsElevated > 0;
    unsafe { CloseHandle(token) };
    elevated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_strings_split_at_the_terminator() {
        let units: Vec<u16> = "1\0Counter\0\0\0".encode_utf16().collect();
        assert_eq!(reg_multi_sz_to_string_array(&units), vec!["1", "Counter"]);
        assert!(reg_multi_sz_to_string_array(&[0, 0]).is_empty());
    }

    #[test]
    fn registry_keys_and_values_can_be_read() {
        let keys = registry_get_keys(ROOT_LOCAL_MACHINE, "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\").ok().unwrap();
        assert!(!keys.is_empty());
        let vendor = registry_get_string_value(ROOT_LOCAL_MACHINE, &format!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\{}", keys[0]), "VendorIdentifier").ok().unwrap();
        assert!(!vendor.is_empty());
        assert!(!registry_value_exists(ROOT_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "NoSuchValueName").ok().unwrap());
        assert!(registry_value_exists(ROOT_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "ProductName").ok().unwrap());
    }
}
