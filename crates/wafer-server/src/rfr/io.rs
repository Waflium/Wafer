use std::io::{self, Write};
use std::path::{Component, Path};

use super::runtime::{FILE_READ, FILE_WRITE};
use super::{Value, ticks};

pub fn path_string(path: &Path) -> String {
    let mut out = String::new();
    for component in path.components() {
        let part = match component {
            Component::Prefix(prefix) => {
                out.push_str(&prefix.as_os_str().to_string_lossy());
                continue;
            }
            Component::RootDir => {
                out.push(std::path::MAIN_SEPARATOR);
                continue;
            }
            Component::CurDir => ".".into(),
            Component::ParentDir => "..".into(),
            Component::Normal(part) => part.to_string_lossy(),
        };
        if !out.is_empty() && !out.ends_with(std::path::MAIN_SEPARATOR) {
            out.push(std::path::MAIN_SEPARATOR);
        }
        out.push_str(&part);
    }
    out
}

pub fn absolute_path_string(path: &Path) -> String {
    if path.is_absolute() {
        return path_string(path);
    }
    match std::env::current_dir() {
        Ok(directory) => format!("{}{}{}", path_string(&directory), std::path::MAIN_SEPARATOR, path_string(path)),
        Err(_) => path_string(path),
    }
}

pub fn file_read(path: &Path, start_ticks: i64, bytes_read: i64, end_of_file: bool) {
    if FILE_READ.is_enabled() {
        FILE_READ.commit(start_ticks, ticks(), &[Value::String(&absolute_path_string(path)), Value::Long(bytes_read), Value::Boolean(end_of_file)]);
    }
}

pub fn file_write(path: Option<&str>, start_ticks: i64, bytes_written: i64) {
    if FILE_WRITE.is_enabled() {
        FILE_WRITE.commit(start_ticks, ticks(), &[path.map_or(Value::NullString, Value::String), Value::Long(bytes_written)]);
    }
}

pub struct RecordedWrite<W> {
    inner: W,
    path: Option<String>,
}

impl<W> RecordedWrite<W> {
    pub fn new(inner: W, path: Option<String>) -> RecordedWrite<W> {
        RecordedWrite { inner, path }
    }

    pub fn get_ref(&self) -> &W {
        &self.inner
    }
}

impl<W: Write> Write for RecordedWrite<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let start = ticks();
        let written = self.inner.write(buf)?;
        file_write(self.path.as_deref(), start, written as i64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
