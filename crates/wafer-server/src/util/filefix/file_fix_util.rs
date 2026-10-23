use std::io;

use crate::util::java_nio::{CopyOption, java_io_error};

use super::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use super::virtualfilesystem::files;

pub const NAMESPACE_PATTERN: &str = "([a-z0-9_.-]+)";

pub fn move_file(base_directory: &CopyOnWriteFSPath, from: &str, to: &str) -> io::Result<()> {
    let from_absolute = base_directory.resolve_str(from);
    if files::exists(&from_absolute) {
        let to_absolute = base_directory.resolve_str(to);
        if files::exists(&to_absolute) {
            return Err(java_io_error(
                io::ErrorKind::AlreadyExists,
                "java.nio.file.FileAlreadyExistsException",
                format!("Cannot move file from {from} to {to}: Target already exists"),
            ));
        }
        files::create_directories_safe(&to_absolute.get_parent().expect("parent"))?;
        files::move_(&from_absolute, &to_absolute, &[CopyOption::CopyAttributes])?;
    }
    Ok(())
}

pub fn delete_file_or_empty_directory(base_directory: &CopyOnWriteFSPath, file: &str) -> io::Result<()> {
    let to_delete = base_directory.resolve_str(file);
    if files::exists(&to_delete) {
        if files::is_directory(&to_delete) {
            let result = (|| -> io::Result<bool> {
                let listed = files::list(&to_delete)?;
                if listed.len() == 1 && listed[0].get_file_name().is_some_and(|name| name.to_string() == ".DS_Store") {
                    if !files::delete_if_exists(&listed[0])? {
                        warn!("Failed to delete file '{}' at '{}'", listed[0], to_delete);
                    }
                }
                let remaining = files::list(&to_delete)?;
                if !remaining.is_empty() {
                    let content: Vec<String> = remaining.iter().map(|path| path.to_string()).collect();
                    warn!("Failed to delete directory '{}', as it's not empty. Content: [{}]", to_delete, content.join(", "));
                    return Ok(true);
                }
                Ok(false)
            })();
            match result {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(e) => {
                    warn!("Failed to delete directory '{}' because {}", to_delete, e);
                    return Ok(());
                }
            }
        }
        files::delete_if_exists(&to_delete)?;
    }
    Ok(())
}
