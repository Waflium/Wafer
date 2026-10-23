use super::copy_on_write_file_system::CopyOnWriteFileSystem;

pub struct CopyOnWriteFileStore {
    name: String,
    fs: CopyOnWriteFileSystem,
}

impl CopyOnWriteFileStore {
    pub fn new(name: String, fs: CopyOnWriteFileSystem) -> CopyOnWriteFileStore {
        CopyOnWriteFileStore { name, fs }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn type_(&self) -> &'static str {
        "copy-on-write"
    }

    pub fn is_read_only(&self) -> bool {
        false
    }

    pub fn file_system(&self) -> &CopyOnWriteFileSystem {
        &self.fs
    }

    pub fn supports_file_attribute_view(&self, name: &str) -> bool {
        name == "basic"
    }
}
