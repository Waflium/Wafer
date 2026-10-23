use super::file_fix_exception::FileFixException;
use super::file_system_capabilities::FileSystemCapabilities;

#[derive(Debug)]
pub struct AtomicMoveNotSupportedFileFixException {
    pub(crate) file_system_capabilities: FileSystemCapabilities,
}

impl AtomicMoveNotSupportedFileFixException {
    pub fn new(file_system_capabilities: FileSystemCapabilities) -> FileFixException {
        FileFixException::AtomicMoveNotSupported(AtomicMoveNotSupportedFileFixException { file_system_capabilities })
    }
}
