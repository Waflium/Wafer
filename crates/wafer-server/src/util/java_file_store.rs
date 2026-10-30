use crate::util::java_stack;
use crate::util::java_throwable::JavaThrowable;

pub struct Space {
    pub usable: u64,
    pub total: u64,
}

pub enum FileStoreError {
    InvalidPath(JavaThrowable),
    NoSuchFile,
    Other(JavaThrowable),
}

fn invalid_path(reason: String, input: &str) -> FileStoreError {
    FileStoreError::InvalidPath(JavaThrowable::new(format!("java.nio.file.InvalidPathException: {reason}: {input}"), java_stack::capture(&[])))
}

fn check_path(path: &str) -> Result<(), FileStoreError> {
    if path.contains('\0') {
        return Err(invalid_path("Nul character not allowed".to_string(), path));
    }
    #[cfg(windows)]
    for (index, character) in path.chars().enumerate() {
        let is_drive_separator = character == ':' && index == 1;
        let is_verbatim_prefix = character == '?' && index == 2 && path.starts_with("\\\\?\\");
        let is_illegal = "<>:\"|?*".contains(character) && !is_drive_separator && !is_verbatim_prefix;
        if is_illegal || (character as u32) < 32 {
            return Err(invalid_path(format!("Illegal char <{character}> at index {index}"), path));
        }
    }
    Ok(())
}

fn existing_path(path: &str) -> Result<std::path::PathBuf, FileStoreError> {
    check_path(path)?;
    let target = if path.is_empty() { "." } else { path };
    match std::fs::canonicalize(target) {
        Ok(canonical) => Ok(canonical),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(FileStoreError::NoSuchFile),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(FileStoreError::Other(JavaThrowable::new(format!("java.nio.file.AccessDeniedException: {path}"), java_stack::capture(&[]))))
        }
        Err(error) => Err(FileStoreError::Other(JavaThrowable::new(format!("java.nio.file.FileSystemException: {path}: {error}"), java_stack::capture(&[])))),
    }
}

#[cfg(windows)]
pub fn get_space(path: &str) -> Result<Space, FileStoreError> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumePathNameW};

    let canonical = existing_path(path)?;
    let wide: Vec<u16> = std::os::windows::ffi::OsStrExt::encode_wide(canonical.as_os_str()).chain(std::iter::once(0)).collect();
    let mut root = vec![0u16; wide.len().max(261)];
    if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
        let code = unsafe { GetLastError() };
        return Err(FileStoreError::Other(JavaThrowable::new(format!("java.nio.file.FileSystemException: {path}: error {code}"), java_stack::capture(&[]))));
    }
    let (mut usable, mut total, mut free) = (0u64, 0u64, 0u64);
    if unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut usable, &mut total, &mut free) } == 0 {
        let code = unsafe { GetLastError() };
        return Err(FileStoreError::Other(JavaThrowable::new(format!("java.nio.file.FileSystemException: {path}: error {code}"), java_stack::capture(&[]))));
    }
    Ok(Space { usable, total })
}

#[cfg(unix)]
pub fn get_space(path: &str) -> Result<Space, FileStoreError> {
    let canonical = existing_path(path)?;
    let c_path = std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(canonical.as_os_str())).map_err(|_| invalid_path("Nul character not allowed".to_string(), path))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        let error = std::io::Error::last_os_error();
        return Err(FileStoreError::Other(JavaThrowable::new(format!("java.nio.file.FileSystemException: {path}: {error}"), java_stack::capture(&[]))));
    }
    let block_size = stat.f_frsize as u64;
    Ok(Space { usable: stat.f_bavail as u64 * block_size, total: stat.f_blocks as u64 * block_size })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_working_directory_has_space() {
        let space = get_space("").ok().unwrap();
        assert!(space.total > 0 && space.usable <= space.total);
    }

    #[test]
    fn missing_paths_are_reported() {
        assert!(matches!(get_space("this-folder-does-not-exist-anywhere"), Err(FileStoreError::NoSuchFile)));
    }

    #[test]
    fn nul_characters_are_invalid_paths() {
        assert!(matches!(get_space("a\0b"), Err(FileStoreError::InvalidPath(_))));
    }
}
