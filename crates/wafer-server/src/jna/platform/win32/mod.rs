pub mod advapi32_util;
pub mod kernel32_util;
pub mod pdh_util;
pub mod win32_exception;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(value: &[u16]) -> String {
    String::from_utf16_lossy(value)
}

pub fn from_wide_nul(value: &[u16]) -> String {
    from_wide(&value[..value.iter().position(|&unit| unit == 0).unwrap_or(value.len())])
}
