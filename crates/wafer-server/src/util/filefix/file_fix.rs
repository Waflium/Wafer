use std::sync::Arc;

use crate::datafixers::data_fix_utils;
use crate::datafixers::schemas::schema::Schema;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;

use super::access::file_access_provider::FileAccessProvider;
use super::canceled_file_fix_exception::CanceledFileFixException;
use super::file_fix_exception::FileFixFailure;
use super::operations::file_fix_operation::FileFixOperation;
use super::operations::modify_content::{FixFunction, ModifyContent};
use super::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;

pub struct FileFix {
    schema: Arc<Schema>,
    file_fix_operations: Vec<Box<dyn FileFixOperation>>,
}

impl FileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        FileFix { schema: schema.clone(), file_fix_operations: Vec::new() }
    }

    pub fn get_schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    pub fn get_version(&self) -> i32 {
        data_fix_utils::get_version(self.schema.version_key)
    }

    pub fn add_file_fix_operation(&mut self, file_fix_operation: impl FileFixOperation + 'static) {
        self.file_fix_operations.push(Box::new(file_fix_operation));
    }

    pub fn add_file_content_fix(&mut self, file_access_function: impl FnOnce(&Arc<FileAccessProvider>) -> FixFunction) {
        let file_access_provider = FileAccessProvider::new(self.get_version());
        let fix_function = file_access_function(&file_access_provider);
        file_access_provider.freeze();
        self.file_fix_operations.push(Box::new(ModifyContent::new(file_access_provider, fix_function)));
    }

    pub fn count_file_operations(&self) -> usize {
        self.file_fix_operations.len()
    }

    pub fn run_fix_operations(&self, base_folder: &CopyOnWriteFSPath, upgrade_progress: &UpgradeProgress) -> Result<(), FileFixFailure> {
        for file_fix_operation in &self.file_fix_operations {
            if upgrade_progress.is_canceled() {
                return Err(CanceledFileFixException::new().into());
            }
            file_fix_operation.fix(base_folder, upgrade_progress)?;
            upgrade_progress.increment_finished_operations();
        }
        Ok(())
    }
}
