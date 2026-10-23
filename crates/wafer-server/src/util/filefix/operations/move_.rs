use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::file_fix_util;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::file_fix_operation::FileFixOperation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    from: String,
    to: String,
}

impl Move {
    pub fn new(from: &str, to: &str) -> Move {
        Move { from: from.to_string(), to: to.to_string() }
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn relative(&self, source_folder: &str, target_folder: &str) -> Move {
        Move { from: format!("{source_folder}/{}", self.from), to: format!("{target_folder}/{}", self.to) }
    }
}

impl FileFixOperation for Move {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, _upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        file_fix_util::move_file(base_directory, &self.from, &self.to)?;
        Ok(())
    }
}
