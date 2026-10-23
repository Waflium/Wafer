use std::sync::Arc;

use crate::util::filefix::access::file_access_provider::FileAccessProvider;
use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::file_fix_operation::FileFixOperation;

pub type FixFunction = Box<dyn Fn(&UpgradeProgress) -> Result<(), FileFixFailure> + Send + Sync>;

pub struct ModifyContent {
    file_access_provider: Arc<FileAccessProvider>,
    fix_function: FixFunction,
}

impl ModifyContent {
    pub fn new(file_access_provider: Arc<FileAccessProvider>, fix_function: FixFunction) -> ModifyContent {
        ModifyContent { file_access_provider, fix_function }
    }
}

impl FileFixOperation for ModifyContent {
    fn fix(&self, base_directory: &CopyOnWriteFSPath, upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        let previous = self.file_access_provider.set_base_directory(Some(base_directory.clone()));
        let result = (self.fix_function)(upgrade_progress);
        self.file_access_provider.set_base_directory(previous);
        self.file_access_provider.close();
        result
    }
}
