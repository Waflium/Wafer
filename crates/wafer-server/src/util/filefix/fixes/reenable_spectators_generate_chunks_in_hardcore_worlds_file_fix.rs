use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references;
use crate::util::filefix::access::compressed_nbt::CompressedNbtFile;
use crate::util::filefix::access::file_relation::FileRelation;
use crate::util::filefix::access::file_resource_types::FileResourceTypes;
use crate::util::filefix::file_fix::FileFix;
use crate::util::worldupdate::upgrade_progress::Type;

pub struct ReenableSpectatorsGenerateChunksInHardcoreWorldsFileFix;

impl ReenableSpectatorsGenerateChunksInHardcoreWorldsFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        file_fix.add_file_content_fix(|files| {
            let level_dat = files.get_file_access(FileResourceTypes::level_dat(), FileRelation::origin().for_file("level.dat"));
            let game_rules = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_GAME_RULES),
                FileRelation::data().for_file("minecraft/game_rules.dat"),
            );
            Box::new(move |upgrade_progress| {
                upgrade_progress.set_type(Type::Files);
                let level_dat_data = level_dat.get_only_file().read()?;
                if let Some(data) = level_dat_data {
                    let was_ever_loaded_in_singleplayer = data.get("singleplayer_uuid").is_present();
                    if was_ever_loaded_in_singleplayer {
                        let hardcore = data.get("difficulty_settings").or_else_empty_map().get("hardcore").as_boolean_or(false);
                        if hardcore {
                            let game_rules_file = game_rules.get_only_file();
                            if let Some(data_tag) = game_rules_file.read()? {
                                game_rules_file.write(&data_tag.update("minecraft:spectators_generate_chunks", |old| old.create_boolean(true)));
                            }
                        }
                    }
                }
                Ok(())
            })
        });
        file_fix
    }
}
