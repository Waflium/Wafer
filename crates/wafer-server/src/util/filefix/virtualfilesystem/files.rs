use std::fs::File;
use std::io;

use crate::util::java_nio::{CopyOption, java_io_error};

use super::copy_on_write_fs_path::CopyOnWriteFSPath;
use super::copy_on_write_fs_provider::OpenOptions;

pub fn exists(path: &CopyOnWriteFSPath) -> bool {
    path.get_file_system().provider().check_access(path).is_ok()
}

pub fn is_directory(path: &CopyOnWriteFSPath) -> bool {
    path.get_file_system().provider().read_attributes(path).is_ok_and(|attributes| attributes.directory)
}

pub fn is_regular_file(path: &CopyOnWriteFSPath) -> bool {
    path.get_file_system().provider().read_attributes(path).is_ok_and(|attributes| attributes.regular_file)
}

pub fn list(dir: &CopyOnWriteFSPath) -> io::Result<Vec<CopyOnWriteFSPath>> {
    dir.get_file_system().provider().new_directory_stream(dir, |_| true)
}

pub fn move_(source: &CopyOnWriteFSPath, target: &CopyOnWriteFSPath, options: &[CopyOption]) -> io::Result<()> {
    source.get_file_system().provider().move_(source, target, options)
}

pub fn delete_if_exists(path: &CopyOnWriteFSPath) -> io::Result<bool> {
    match path.get_file_system().provider().delete(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn create_directory(dir: &CopyOnWriteFSPath) -> io::Result<()> {
    dir.get_file_system().provider().create_directory(dir)
}

fn create_and_check_is_directory(dir: &CopyOnWriteFSPath) -> io::Result<()> {
    match create_directory(dir) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            if is_directory(dir) {
                Ok(())
            } else {
                Err(e)
            }
        }
        other => other,
    }
}

pub fn create_directories(dir: &CopyOnWriteFSPath) -> io::Result<()> {
    match create_and_check_is_directory(dir) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(e),
        Err(_) => {}
    }
    let dir = dir.to_absolute_path();
    let mut parent = dir.get_parent();
    while let Some(candidate) = &parent {
        match dir.get_file_system().provider().check_access(candidate) {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        parent = candidate.get_parent();
    }
    let Some(parent) = parent else {
        return Err(java_io_error(io::ErrorKind::Other, "java.nio.file.FileSystemException", format!("{dir}: Unable to determine if root directory exists")));
    };
    let mut child = parent.clone();
    let relative = parent.relativize(&dir);
    for index in 0..relative.get_name_count() {
        child = child.resolve(&relative.get_name(index));
        create_and_check_is_directory(&child)?;
    }
    Ok(())
}

pub fn create_directories_safe(dir: &CopyOnWriteFSPath) -> io::Result<()> {
    if exists(dir) { create_directories(&dir.to_real_path()?) } else { create_directories(dir) }
}

pub fn new_input_stream(path: &CopyOnWriteFSPath) -> io::Result<File> {
    path.get_file_system().provider().new_input_stream(path)
}

pub fn new_output_stream(path: &CopyOnWriteFSPath, options: &OpenOptions) -> io::Result<File> {
    path.get_file_system().provider().new_output_stream(path, options)
}
