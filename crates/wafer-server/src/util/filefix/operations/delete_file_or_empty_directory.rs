use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::file_fix_util;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::file_fix_operation::FileFixOperation;

pub struct DeleteFileOrEmptyDirectory {
    target: String,
}

impl DeleteFileOrEmptyDirectory {
    pub fn new(target: &str) -> DeleteFileOrEmptyDirectory {
        DeleteFileOrEmptyDirectory { target: target.to_string() }
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

impl FileFixOperation for DeleteFileOrEmptyDirectory {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, _upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        file_fix_util::delete_file_or_empty_directory(base_directory, &self.target)?;
        Ok(())
    }
}
