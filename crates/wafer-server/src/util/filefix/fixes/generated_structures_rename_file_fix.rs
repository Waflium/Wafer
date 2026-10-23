use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::filefix::access::file_relation::FileRelation;
use crate::util::filefix::file_fix::FileFix;
use crate::util::filefix::operations::file_fix_operations::FileFixOperations;

pub struct GeneratedStructuresRenameFileFix;

impl GeneratedStructuresRenameFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        file_fix.add_file_fix_operation(FileFixOperations::apply_in_folders(
            FileRelation::generated_namespaces(),
            vec![Box::new(FileFixOperations::move_("structures", "structure"))],
        ));
        file_fix
    }
}
