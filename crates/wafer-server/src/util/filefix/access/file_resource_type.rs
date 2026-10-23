use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;

use super::AutoCloseable;

pub struct FileResourceType<T: AutoCloseable> {
    factory: Box<dyn Fn(CopyOnWriteFSPath, i32) -> T + Send + Sync>,
}

impl<T: AutoCloseable> FileResourceType<T> {
    pub fn new(factory: impl Fn(CopyOnWriteFSPath, i32) -> T + Send + Sync + 'static) -> FileResourceType<T> {
        FileResourceType { factory: Box::new(factory) }
    }

    pub fn create(&self, path: CopyOnWriteFSPath, data_version: i32) -> T {
        (self.factory)(path, data_version)
    }
}
