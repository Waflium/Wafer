use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, Timelike};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::rfr::io::path_string;

pub struct FileZipper {
    output_file: PathBuf,
    temp_file: PathBuf,
    writer: ZipWriter<File>,
    directories: HashSet<String>,
}

fn zip_error(error: zip::result::ZipError) -> io::Error {
    match error {
        zip::result::ZipError::Io(error) => error,
        other => io::Error::other(other.to_string()),
    }
}

impl FileZipper {
    pub fn new(output_file: &Path) -> io::Result<FileZipper> {
        let mut temp_name = output_file.file_name().unwrap_or_default().to_os_string();
        temp_name.push("_tmp");
        let temp_file = output_file.with_file_name(temp_name);
        let writer = ZipWriter::new(File::create(&temp_file)?);
        Ok(FileZipper { output_file: output_file.to_path_buf(), temp_file, writer, directories: HashSet::new() })
    }

    fn options() -> SimpleFileOptions {
        let now = chrono::Local::now();
        let modified = zip::DateTime::from_date_and_time(now.year() as u16, now.month() as u8, now.day() as u8, now.hour() as u8, now.minute() as u8, now.second() as u8).unwrap_or_default();
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).last_modified_time(modified)
    }

    fn create_directories(&mut self, parent: &str) -> io::Result<()> {
        if parent.is_empty() {
            return Ok(());
        }
        let mut current = String::new();
        for segment in parent.split('/') {
            current.push_str(segment);
            current.push('/');
            if self.directories.insert(current.clone()) {
                self.writer.add_directory(current.clone(), Self::options()).map_err(zip_error)?;
            }
        }
        Ok(())
    }

    fn add_entry(&mut self, destination_relative_path: &str, content: &[u8]) -> io::Result<()> {
        if let Some((parent, _)) = destination_relative_path.rsplit_once('/') {
            self.create_directories(parent)?;
        }
        self.writer.start_file(destination_relative_path, Self::options()).map_err(zip_error)?;
        self.writer.write_all(content)
    }

    pub fn add_string(&mut self, destination_relative_path: &str, content: &str) -> io::Result<()> {
        self.add_entry(destination_relative_path, content.as_bytes())
    }

    pub fn add_directory_contents(&mut self, path: &Path) -> io::Result<()> {
        let mut source_files = Vec::new();
        collect_regular_files(path, &mut source_files)?;
        for source_file in source_files {
            let relative = source_file.strip_prefix(path).map_err(io::Error::other)?;
            let relative = relative.components().map(|component| component.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            let content = fs::read(&source_file)?;
            self.add_entry(&relative, &content)?;
        }
        Ok(())
    }

    pub fn close(self) -> io::Result<()> {
        let FileZipper { output_file, temp_file, writer, .. } = self;
        drop(writer.finish().map_err(zip_error)?);
        fs::rename(&temp_file, &output_file)?;
        info!("Compressed to {}", path_string(&output_file));
        Ok(())
    }
}

fn collect_regular_files(directory: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(directory)?.collect::<io::Result<_>>()?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_uppercase());
    for entry in entries {
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_regular_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_appear_before_their_first_file_in_walk_order() {
        let dir = std::env::temp_dir().join(format!("wafer-zipper-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("report/levels/overworld")).unwrap();
        fs::create_dir_all(dir.join("report/metrics")).unwrap();
        fs::write(dir.join("report/levels/overworld/stats.txt"), "stats").unwrap();
        fs::write(dir.join("report/metrics/cpu.csv"), "cpu").unwrap();
        fs::write(dir.join("report/profiling.txt"), "profile").unwrap();
        fs::create_dir_all(dir.join("report/empty")).unwrap();
        let zip_path = dir.join("out.zip");
        let mut zipper = FileZipper::new(&zip_path).unwrap();
        zipper.add_string("system.txt", "system").unwrap();
        zipper.add_directory_contents(&dir.join("report")).unwrap();
        zipper.close().unwrap();
        let archive = zip::ZipArchive::new(File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<&str> = archive.file_names().collect();
        assert_eq!(names, ["system.txt", "levels/", "levels/overworld/", "levels/overworld/stats.txt", "metrics/", "metrics/cpu.csv", "profiling.txt"]);
        assert!(!dir.join("out.zip_tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
