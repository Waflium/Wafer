use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;

use super::AutoCloseable;
use super::file_access::FileAccess;
use super::file_relation::FileRelation;
use super::file_resource_type::FileResourceType;

pub struct FileAccessProvider {
    accessed_files: Mutex<Vec<Arc<dyn AutoCloseable>>>,
    base_directory: Mutex<Option<CopyOnWriteFSPath>>,
    data_version: i32,
    frozen: AtomicBool,
}

impl FileAccessProvider {
    pub fn new(data_version: i32) -> Arc<FileAccessProvider> {
        Arc::new(FileAccessProvider {
            accessed_files: Mutex::new(Vec::new()),
            base_directory: Mutex::new(None),
            data_version,
            frozen: AtomicBool::new(false),
        })
    }

    pub fn get_file_access<T: AutoCloseable + 'static>(
        self: &Arc<FileAccessProvider>,
        type_: FileResourceType<T>,
        file_relation: FileRelation,
    ) -> Arc<FileAccess<T>> {
        if self.frozen.load(Ordering::SeqCst) {
            panic!("Cannot request new file access here.");
        }
        let file_access = Arc::new(FileAccess::new(self.clone(), type_, file_relation));
        self.accessed_files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(file_access.clone());
        file_access
    }

    pub fn freeze(&self) {
        self.frozen.store(true, Ordering::SeqCst);
    }

    pub fn base_directory(&self) -> Option<CopyOnWriteFSPath> {
        self.base_directory.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    pub fn set_base_directory(&self, base_directory: Option<CopyOnWriteFSPath>) -> Option<CopyOnWriteFSPath> {
        std::mem::replace(&mut *self.base_directory.lock().unwrap_or_else(|poisoned| poisoned.into_inner()), base_directory)
    }

    pub fn data_version(&self) -> i32 {
        self.data_version
    }

    pub fn close(&self) {
        let accessed_files: Vec<Arc<dyn AutoCloseable>> = self.accessed_files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
        for accessed_file in accessed_files {
            let _ = accessed_file.close();
        }
    }
}
