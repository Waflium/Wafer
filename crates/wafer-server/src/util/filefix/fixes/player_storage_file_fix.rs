use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::filefix::file_fix::FileFix;
use crate::util::filefix::operations::file_fix_operations::FileFixOperations;

pub struct PlayerStorageFileFix;

impl PlayerStorageFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        file_fix.add_file_fix_operation(FileFixOperations::move_("advancements", "players/advancements"));
        file_fix.add_file_fix_operation(FileFixOperations::move_("playerdata", "players/data"));
        file_fix.add_file_fix_operation(FileFixOperations::move_("stats", "players/stats"));
        file_fix
    }
}
