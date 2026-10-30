use std::fmt;
use std::io;

use crate::crash_report::CrashReport;
use crate::util::java_throwable::JavaThrowable;

use super::aborted_file_fix_exception::AbortedFileFixException;
use super::atomic_move_not_supported_file_fix_exception::AtomicMoveNotSupportedFileFixException;
use super::canceled_file_fix_exception::CanceledFileFixException;
use super::failed_cleanup_file_fix_exception::FailedCleanupFileFixException;
use super::file_system_capabilities::FileSystemCapabilities;

#[derive(Debug)]
pub enum FileFixException {
    Aborted(AbortedFileFixException),
    AtomicMoveNotSupported(AtomicMoveNotSupportedFileFixException),
    Canceled(CanceledFileFixException),
    FailedCleanup(FailedCleanupFileFixException),
}

impl FileFixException {
    pub fn class_name(&self) -> &'static str {
        match self {
            FileFixException::Aborted(_) => "net.minecraft.util.filefix.AbortedFileFixException",
            FileFixException::AtomicMoveNotSupported(_) => "net.minecraft.util.filefix.AtomicMoveNotSupportedFileFixException",
            FileFixException::Canceled(_) => "net.minecraft.util.filefix.CanceledFileFixException",
            FileFixException::FailedCleanup(_) => "net.minecraft.util.filefix.FailedCleanupFileFixException",
        }
    }

    pub fn cause(&self) -> Option<&io::Error> {
        match self {
            FileFixException::Aborted(exception) => exception.cause.as_ref(),
            FileFixException::AtomicMoveNotSupported(_) => None,
            FileFixException::Canceled(_) => None,
            FileFixException::FailedCleanup(exception) => Some(&exception.cause),
        }
    }

    pub fn file_system_capabilities(&self) -> Option<FileSystemCapabilities> {
        match self {
            FileFixException::Aborted(exception) => exception.file_system_capabilities,
            FileFixException::AtomicMoveNotSupported(exception) => Some(exception.file_system_capabilities),
            FileFixException::Canceled(_) => None,
            FileFixException::FailedCleanup(exception) => Some(exception.file_system_capabilities),
        }
    }

    pub fn create_crash_report(&self) -> CrashReport {
        let mut crash_report = CrashReport::new("Upgrading world failed with errors", self.to_string(), Vec::new());
        let capabilities = self.file_system_capabilities();
        let fs_capabilities = crash_report.add_category("File system capabilities");
        fs_capabilities.set_detail("Hard Links", capabilities.map_or("null".to_string(), |c| c.hard_links().to_string()));
        fs_capabilities.set_detail("Atomic Move", capabilities.map_or("null".to_string(), |c| c.atomic_move().to_string()));
        match self {
            FileFixException::Aborted(exception) => {
                let failed_reverts = crash_report.add_category("Moves that failed to revert");
                for (i, not_reverted_move) in exception.not_reverted_moves.iter().enumerate() {
                    failed_reverts.set_detail(&i.to_string(), format!("{} -> {}", not_reverted_move.from().display(), not_reverted_move.to().display()));
                }
            }
            FileFixException::FailedCleanup(exception) => {
                let world_upgrade = crash_report.add_category("World upgrade");
                world_upgrade.set_detail("New Name", exception.new_world_folder_name.clone());
            }
            FileFixException::AtomicMoveNotSupported(_) | FileFixException::Canceled(_) => {}
        }
        crash_report
    }
}

impl fmt::Display for FileFixException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cause() {
            Some(cause) => write!(f, "{}: {}", self.class_name(), cause),
            None => f.write_str(self.class_name()),
        }
    }
}

impl std::error::Error for FileFixException {}

#[derive(Debug)]
pub struct UncheckedException {
    pub description: String,
    pub cause: Option<String>,
}

impl UncheckedException {
    pub fn new(description: impl Into<String>) -> UncheckedException {
        UncheckedException { description: description.into(), cause: None }
    }

    pub fn completion(cause: impl Into<String>) -> UncheckedException {
        let cause = cause.into();
        let message = cause.lines().next().unwrap_or_default();
        UncheckedException { description: format!("java.util.concurrent.CompletionException: {message}"), cause: Some(cause) }
    }
}

impl fmt::Display for UncheckedException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description)?;
        match &self.cause {
            Some(cause) => write!(f, "\nCaused by: {cause}"),
            None => Ok(()),
        }
    }
}

#[derive(Debug)]
pub enum FileFixFailure {
    Io(io::Error),
    Throwable(JavaThrowable),
    Exception(FileFixException),
    Unchecked(UncheckedException),
}

impl From<UncheckedException> for FileFixFailure {
    fn from(exception: UncheckedException) -> FileFixFailure {
        FileFixFailure::Unchecked(exception)
    }
}

impl fmt::Display for FileFixFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileFixFailure::Unchecked(e) => write!(f, "{e}"),
            FileFixFailure::Io(e) => write!(f, "java.io.UncheckedIOException: {e}\nCaused by: {e}"),
            FileFixFailure::Throwable(throwable) => f.write_str(throwable.print_stack_trace().trim_end()),
            FileFixFailure::Exception(e) => match e.cause() {
                Some(cause) => write!(f, "{e}\nCaused by: {cause}"),
                None => write!(f, "{e}"),
            },
        }
    }
}

impl From<io::Error> for FileFixFailure {
    fn from(error: io::Error) -> FileFixFailure {
        FileFixFailure::Io(error)
    }
}

impl From<FileFixException> for FileFixFailure {
    fn from(exception: FileFixException) -> FileFixFailure {
        FileFixFailure::Exception(exception)
    }
}
