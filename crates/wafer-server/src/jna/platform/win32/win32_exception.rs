use crate::util::java_stack;
use crate::util::java_throwable::JavaThrowable;

pub struct Win32Exception {
    pub error_code: i32,
    pub throwable: JavaThrowable,
}

impl Win32Exception {
    pub fn new(error_code: i32, frames_top_first: &[&'static str]) -> Win32Exception {
        let hr = hresult_from_win32(error_code);
        let message = super::kernel32_util::format_message(hr).unwrap_or_else(|| format!("Unknown error 0x{hr:X}"));
        Win32Exception {
            error_code,
            throwable: JavaThrowable::new(format!("com.sun.jna.platform.win32.Win32Exception: {message}"), java_stack::capture(frames_top_first)),
        }
    }

    pub fn get_hr(&self) -> i32 {
        hresult_from_win32(self.error_code)
    }
}

impl From<Win32Exception> for JavaThrowable {
    fn from(exception: Win32Exception) -> JavaThrowable {
        exception.throwable
    }
}

pub fn hresult_from_win32(code: i32) -> i32 {
    if code <= 0 { code } else { ((code & 0xFFFF) | 0x8007_0000u32 as i32) as i32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win32_codes_become_hresults() {
        assert_eq!(hresult_from_win32(87) as u32, 0x8007_0057);
        assert_eq!(hresult_from_win32(0), 0);
        assert_eq!(hresult_from_win32(-2147024809), -2147024809);
    }

    #[test]
    fn exceptions_carry_the_system_message() {
        let exception = Win32Exception::new(5, &["com.sun.jna.platform.win32.Advapi32Util.registryGetKeys(Advapi32Util.java:2043)"]);
        assert!(exception.throwable.description.starts_with("com.sun.jna.platform.win32.Win32Exception: "));
        assert_eq!(exception.throwable.stack_trace[0], "com.sun.jna.platform.win32.Advapi32Util.registryGetKeys(Advapi32Util.java:2043)");
        assert_eq!(exception.get_hr() as u32, 0x8007_0005);
    }
}
