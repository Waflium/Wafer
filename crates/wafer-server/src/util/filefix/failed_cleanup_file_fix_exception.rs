use std::io;

use super::file_fix_exception::FileFixException;
use super::file_system_capabilities::FileSystemCapabilities;

#[derive(Debug)]
pub struct FailedCleanupFileFixException {
    pub(crate) cause: io::Error,
    pub(crate) new_world_folder_name: String,
    pub(crate) file_system_capabilities: FileSystemCapabilities,
}

impl FailedCleanupFileFixException {
    pub fn new(cause: io::Error, new_world_folder_name: String, file_system_capabilities: FileSystemCapabilities) -> FileFixException {
        FileFixException::FailedCleanup(FailedCleanupFileFixException { cause, new_world_folder_name, file_system_capabilities })
    }

    pub fn new_world_folder_name(&self) -> &str {
        &self.new_world_folder_name
    }
}
