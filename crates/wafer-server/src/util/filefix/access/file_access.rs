use std::io;
use std::sync::{Arc, Mutex};

use super::AutoCloseable;
use super::file_access_provider::FileAccessProvider;
use super::file_relation::FileRelation;
use super::file_resource_type::FileResourceType;

pub struct FileAccess<T: AutoCloseable> {
    file_access_provider: Arc<FileAccessProvider>,
    type_: FileResourceType<T>,
    file_relation: FileRelation,
    files: Mutex<Option<Vec<Arc<T>>>>,
}

impl<T: AutoCloseable> FileAccess<T> {
    pub fn new(file_access_provider: Arc<FileAccessProvider>, type_: FileResourceType<T>, file_relation: FileRelation) -> FileAccess<T> {
        FileAccess { file_access_provider, type_, file_relation, files: Mutex::new(None) }
    }

    pub fn get(&self) -> Vec<Arc<T>> {
        let mut files = self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if files.is_none() {
            let Some(base_directory) = self.file_access_provider.base_directory() else {
                panic!("Cannot access world files");
            };
            *files = Some(
                self.file_relation
                    .get_paths(&base_directory)
                    .into_iter()
                    .map(|path| Arc::new(self.type_.create(path, self.file_access_provider.data_version())))
                    .collect(),
            );
        }
        files.clone().unwrap_or_default()
    }

    pub fn get_only_file(&self) -> Arc<T> {
        let files = self.get();
        if files.len() != 1 {
            panic!("Trying to get only file, but there are {} files", files.len());
        }
        files[0].clone()
    }
}

impl<T: AutoCloseable> AutoCloseable for FileAccess<T> {
    fn close(&self) -> io::Result<()> {
        let files = self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
        if let Some(files) = files {
            for file in files {
                if let Err(e) = file.close() {
                    error!("Failed to close file: \n{e}");
                }
            }
        }
        Ok(())
    }
}
