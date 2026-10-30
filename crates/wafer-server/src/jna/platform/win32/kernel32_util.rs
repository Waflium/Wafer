use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::System::Diagnostics::Debug::{FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW};

pub fn format_message(code: i32) -> Option<String> {
    let mut buffer: *mut u16 = std::ptr::null_mut();
    let length = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            code as u32,
            0,
            (&mut buffer as *mut *mut u16).cast(),
            0,
            std::ptr::null(),
        )
    };
    if length == 0 || buffer.is_null() {
        return None;
    }
    let message = super::from_wide_nul(unsafe { std::slice::from_raw_parts(buffer, length as usize) });
    unsafe { LocalFree(buffer.cast()) };
    Some(message.trim().to_string())
}
