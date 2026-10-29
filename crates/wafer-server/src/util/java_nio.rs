use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub struct JavaException {
    pub class: &'static str,
    pub message: String,
}

impl fmt::Display for JavaException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class, self.message)
    }
}

impl std::error::Error for JavaException {}

pub fn java_io_error(kind: io::ErrorKind, class: &'static str, message: String) -> io::Error {
    io::Error::new(kind, JavaException { class, message })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CopyOption {
    AtomicMove,
    ReplaceExisting,
    CopyAttributes,
    NoFollowLinks,
}

pub fn for_path(error: io::Error, path: &Path) -> io::Error {
    if error.get_ref().is_some_and(|inner| inner.is::<JavaException>()) {
        return error;
    }
    let message = path.display().to_string();
    match error.kind() {
        io::ErrorKind::NotFound => java_io_error(error.kind(), "java.nio.file.NoSuchFileException", message),
        io::ErrorKind::PermissionDenied => java_io_error(error.kind(), "java.nio.file.AccessDeniedException", message),
        io::ErrorKind::AlreadyExists => java_io_error(error.kind(), "java.nio.file.FileAlreadyExistsException", message),
        io::ErrorKind::DirectoryNotEmpty => java_io_error(error.kind(), "java.nio.file.DirectoryNotEmptyException", message),
        _ => java_io_error(error.kind(), "java.nio.file.FileSystemException", format!("{message}: {}", os_message(&error))),
    }
}

pub fn os_message(error: &io::Error) -> String {
    let message = error.to_string();
    message.split(" (os error").next().unwrap_or(&message).to_string()
}

pub fn is_regular_file(path: &Path) -> bool {
    path.is_file()
}

pub fn delete_if_exists(path: &Path) -> io::Result<bool> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(for_path(e, path)),
    };
    let result = if metadata.is_dir() { std::fs::remove_dir(path) } else { std::fs::remove_file(path) };
    match result {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(for_path(e, path)),
    }
}

pub fn move_file(from: &Path, to: &Path, options: &[CopyOption]) -> io::Result<()> {
    if std::fs::symlink_metadata(from).is_err() {
        return Err(for_path(io::Error::from(io::ErrorKind::NotFound), from));
    }
    let replace = options.contains(&CopyOption::ReplaceExisting);
    let atomic = options.contains(&CopyOption::AtomicMove);
    if !atomic && !replace && std::fs::symlink_metadata(to).is_ok() {
        return Err(for_path(io::Error::from(io::ErrorKind::AlreadyExists), to));
    }
    std::fs::rename(from, to).map_err(|e| {
        let paths = format!("{} -> {}", from.display(), to.display());
        match e.kind() {
            io::ErrorKind::NotFound => java_io_error(e.kind(), "java.nio.file.NoSuchFileException", paths),
            io::ErrorKind::AlreadyExists => java_io_error(e.kind(), "java.nio.file.FileAlreadyExistsException", paths),
            io::ErrorKind::PermissionDenied => java_io_error(e.kind(), "java.nio.file.AccessDeniedException", paths),
            io::ErrorKind::DirectoryNotEmpty => java_io_error(e.kind(), "java.nio.file.DirectoryNotEmptyException", from.display().to_string()),
            _ => java_io_error(e.kind(), "java.nio.file.FileSystemException", format!("{paths}: {}", os_message(&e))),
        }
    })
}
