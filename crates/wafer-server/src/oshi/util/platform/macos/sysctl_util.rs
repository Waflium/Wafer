use std::ffi::CString;

fn sysctl_bytes(name: &str, capacity: usize) -> Option<Vec<u8>> {
    let name = CString::new(name).ok()?;
    let mut buffer = vec![0u8; capacity];
    let mut size = capacity;
    let result = unsafe { libc::sysctlbyname(name.as_ptr(), buffer.as_mut_ptr().cast(), &mut size, std::ptr::null_mut(), 0) };
    if result != 0 {
        return None;
    }
    buffer.truncate(size);
    Some(buffer)
}

pub fn sysctl_string(name: &str, default: &str) -> String {
    match sysctl_bytes(name, 1024) {
        Some(bytes) => String::from_utf8_lossy(&bytes[..bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len())]).into_owned(),
        None => default.to_string(),
    }
}

pub fn sysctl_i32(name: &str, default: i32) -> i32 {
    match sysctl_bytes(name, 4) {
        Some(bytes) if bytes.len() == 4 => i32::from_ne_bytes(bytes.try_into().unwrap_or([0; 4])),
        _ => default,
    }
}

pub fn sysctl_i64(name: &str, default: i64) -> i64 {
    match sysctl_bytes(name, 8) {
        Some(bytes) if bytes.len() == 8 => i64::from_ne_bytes(bytes.try_into().unwrap_or([0; 8])),
        _ => default,
    }
}

pub fn sysctl_struct(name: &str, size: usize) -> Option<Vec<u8>> {
    sysctl_bytes(name, size).filter(|bytes| bytes.len() == size)
}
