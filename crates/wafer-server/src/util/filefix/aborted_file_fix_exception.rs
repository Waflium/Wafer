use std::io;

use super::file_fix_exception::FileFixException;
use super::file_system_capabilities::FileSystemCapabilities;
use super::virtualfilesystem::file_move::FileMove;

#[derive(Debug)]
pub struct AbortedFileFixException {
    pub(crate) cause: Option<io::Error>,
    pub(crate) file_system_capabilities: Option<FileSystemCapabilities>,
    pub(crate) not_reverted_moves: Vec<FileMove>,
}

impl AbortedFileFixException {
    pub fn new(cause: io::Error, not_reverted_moves: Vec<FileMove>, file_system_capabilities: Option<FileSystemCapabilities>) -> FileFixException {
        FileFixException::Aborted(AbortedFileFixException { cause: Some(cause), file_system_capabilities, not_reverted_moves })
    }

    pub fn of(cause: io::Error) -> FileFixException {
        AbortedFileFixException::new(cause, Vec::new(), None)
    }

    pub fn not_reverted_moves(&self) -> &[FileMove] {
        &self.not_reverted_moves
    }
}
