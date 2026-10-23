use std::io;
use std::path::PathBuf;

pub(crate) struct FileNode {
    pub(crate) storage_path: PathBuf,
    pub(crate) is_copy: bool,
}

impl FileNode {
    pub(crate) fn new(storage_path: PathBuf, is_copy: bool) -> FileNode {
        FileNode { storage_path, is_copy }
    }

    pub(crate) fn ensure_copy(&mut self, temporary_file_path: impl FnOnce() -> PathBuf) -> io::Result<()> {
        if !self.is_copy {
            let temp_file = temporary_file_path();
            std::fs::copy(&self.storage_path, &temp_file)?;
            self.storage_path = temp_file;
            self.is_copy = true;
        }
        Ok(())
    }

    pub(crate) fn delete_copy(&self) -> io::Result<()> {
        if self.is_copy {
            match std::fs::remove_file(&self.storage_path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}
