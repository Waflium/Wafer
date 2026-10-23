use crate::util::filefix::access::file_relation::FileRelation;
use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::file_fix_operation::FileFixOperation;

pub struct ApplyInFolders {
    folders: FileRelation,
    file_fix_operations: Vec<Box<dyn FileFixOperation>>,
}

impl ApplyInFolders {
    pub fn new(folders: FileRelation, file_fix_operations: Vec<Box<dyn FileFixOperation>>) -> ApplyInFolders {
        ApplyInFolders { folders, file_fix_operations }
    }
}

impl FileFixOperation for ApplyInFolders {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        for path in self.folders.get_paths(base_directory) {
            for operation in &self.file_fix_operations {
                operation.fix(&path, upgrade_progress)?;
            }
        }
        Ok(())
    }
}
