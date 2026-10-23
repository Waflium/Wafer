use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

pub trait FileFixOperation: Send + Sync {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure>;
}
