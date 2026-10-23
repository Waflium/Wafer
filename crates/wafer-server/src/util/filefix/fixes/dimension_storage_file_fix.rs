use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::filefix::access::file_relation::FileRelation;
use crate::util::filefix::file_fix::FileFix;
use crate::util::filefix::operations::file_fix_operations::FileFixOperations;

pub struct DimensionStorageFileFix;

impl DimensionStorageFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        file_fix.add_file_fix_operation(FileFixOperations::apply_in_folders(
            FileRelation::dimensions_data(),
            vec![
                Box::new(FileFixOperations::move_("chunks.dat", "minecraft/chunk_tickets.dat")),
                Box::new(FileFixOperations::move_("raids.dat", "minecraft/raids.dat")),
                Box::new(FileFixOperations::move_("world_border.dat", "minecraft/world_border.dat")),
            ],
        ));
        file_fix.add_file_fix_operation(FileFixOperations::group_move(
            &[
                ("data", "dimensions/minecraft/overworld/data/minecraft"),
                ("DIM-1/data", "dimensions/minecraft/the_nether/data/minecraft"),
                ("DIM1/data", "dimensions/minecraft/the_end/data/minecraft"),
            ],
            vec![
                FileFixOperations::move_("chunks.dat", "chunk_tickets.dat"),
                FileFixOperations::move_simple("raids.dat"),
                FileFixOperations::move_("raids_end.dat", "raids.dat"),
                FileFixOperations::move_simple("world_border.dat"),
            ],
        ));
        file_fix.add_file_fix_operation(FileFixOperations::delete("DIM1/data"));
        file_fix.add_file_fix_operation(FileFixOperations::delete("DIM-1/data"));
        file_fix.add_file_fix_operation(FileFixOperations::apply_in_folders(
            FileRelation::data(),
            vec![
                Box::new(FileFixOperations::move_("scoreboard.dat", "minecraft/scoreboard.dat")),
                Box::new(FileFixOperations::move_("stopwatches.dat", "minecraft/stopwatches.dat")),
                Box::new(FileFixOperations::move_regex("command_storage_([a-z0-9_.-]+)\\.dat", "$1/command_storage\\.dat")),
                Box::new(FileFixOperations::move_("idcounts.dat", "minecraft/maps/last_id.dat")),
                Box::new(FileFixOperations::move_regex("map_(\\d+)\\.dat", "minecraft/maps/$1\\.dat")),
                Box::new(FileFixOperations::move_("random_sequences.dat", "minecraft/random_sequences.dat")),
            ],
        ));
        file_fix.add_file_fix_operation(FileFixOperations::group_move(
            &[("", "dimensions/minecraft/overworld"), ("DIM-1", "dimensions/minecraft/the_nether"), ("DIM1", "dimensions/minecraft/the_end")],
            vec![FileFixOperations::move_simple("region"), FileFixOperations::move_simple("entities"), FileFixOperations::move_simple("poi")],
        ));
        file_fix.add_file_fix_operation(FileFixOperations::delete("DIM-1"));
        file_fix.add_file_fix_operation(FileFixOperations::delete("DIM1"));
        file_fix
    }
}
