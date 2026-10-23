use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::file_fix_operation::FileFixOperation;
use super::move_::Move;

pub struct GroupMove {
    from_to: Vec<(String, String)>,
    fixers: Vec<Move>,
}

impl GroupMove {
    pub fn new(from_to: Vec<(String, String)>, fixers: Vec<Move>) -> GroupMove {
        GroupMove { from_to, fixers }
    }
}

impl FileFixOperation for GroupMove {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        for (key, value) in &self.from_to {
            for move_operation in &self.fixers {
                let relative = move_operation.relative(key, value);
                relative.fix(base_directory, upgrade_progress)?;
            }
        }
        Ok(())
    }
}
