use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::filefix::file_fix::FileFix;
use crate::util::filefix::operations::file_fix_operations::FileFixOperations;

use super::legacy_structure_file_fix::{END_LEGACY_STRUCTURES, NETHER_LEGACY_STRUCTURES, OVERWORLD_LEGACY_STRUCTURES};

pub struct RemoveObsoleteFilesFileFix;

impl RemoveObsoleteFilesFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        file_fix.add_file_fix_operation(FileFixOperations::delete("data/villages.dat"));
        file_fix.add_file_fix_operation(FileFixOperations::delete("data/villages_end.dat"));
        file_fix.add_file_fix_operation(FileFixOperations::delete("data/villages_nether.dat"));
        file_fix.add_file_fix_operation(FileFixOperations::delete("data/advancements"));
        file_fix.add_file_fix_operation(FileFixOperations::delete("data/functions"));
        let leftover_structures = OVERWORLD_LEGACY_STRUCTURES
            .iter()
            .chain(["Village"].iter())
            .chain(NETHER_LEGACY_STRUCTURES.iter())
            .chain(END_LEGACY_STRUCTURES.iter());
        for leftover_structure in leftover_structures {
            file_fix.add_file_fix_operation(FileFixOperations::delete(&format!("data/{leftover_structure}.dat")));
            file_fix.add_file_fix_operation(FileFixOperations::delete(&format!("data/{leftover_structure}_index.dat")));
        }
        file_fix
    }
}
