use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use super::region_storage_upgrader::{Builder, RegionStorageUpgrader};
use super::upgrade_progress::{Status, Type, UpgradeProgress};
use super::upgrade_status_translator::{StatusMessage, UpgradeStatusTranslator};
use crate::server::notifications::NotificationManager;
use crate::util::datafix::data_fix_types::{DataFixTypes, current_version};
use crate::world::level::dimension::level_stem::LevelStem;

struct Work {
    levels: Vec<(String, &'static str)>,
    erase_cache: bool,
    recreate_region_files: bool,
    level_directory: PathBuf,
    upgrade_progress: Arc<UpgradeProgress>,
}

pub struct WorldUpgrader {
    status_translator: UpgradeStatusTranslator,
    upgrade_progress: Arc<UpgradeProgress>,
}

impl WorldUpgrader {
    pub fn new(level_directory: PathBuf, dimensions: &[(String, LevelStem)], erase_cache: bool, recreate_region_files: bool) -> WorldUpgrader {
        let upgrade_progress = Arc::new(UpgradeProgress::new(NotificationManager::default()));
        let work = Work {
            levels: dimensions.iter().map(|(key, level_stem)| (key.clone(), level_stem.generator.get_type_name_for_data_fixer())).collect(),
            erase_cache,
            recreate_region_files,
            level_directory,
            upgrade_progress: upgrade_progress.clone(),
        };
        std::thread::Builder::new()
            .name("World Upgrader #0".to_string())
            .spawn(move || {
                let progress = work.upgrade_progress.clone();
                if let Err(throwable) = crate::util::catch_throwable(|| work.run()) {
                    error!("Error upgrading world\n{}", throwable.print_stack_trace().trim_end());
                    progress.set_status(Status::Failed);
                    progress.set_finished(true);
                }
            })
            .expect("world upgrader thread");
        WorldUpgrader { status_translator: UpgradeStatusTranslator, upgrade_progress }
    }

    pub fn get_data_fix_context_tag(dimension: &str, generator_type_name: &'static str) -> Nbt {
        crate::server::level::chunk_map::get_chunk_data_fix_context_tag(dimension, Some(generator_type_name))
    }

    pub fn verify_chunk_pos_and_erase_cache(pos: ChunkPos, upgraded_tag: &mut Nbt) -> bool {
        Self::verify_chunk_pos(pos, upgraded_tag);
        let Nbt::Compound(entries) = upgraded_tag else { return false };
        let mut changed = entries.iter().any(|(key, _)| key == "Heightmaps");
        entries.remove_key("Heightmaps");
        changed = changed || entries.iter().any(|(key, _)| key == "isLightOn");
        entries.remove_key("isLightOn");
        if let Some((_, Nbt::List(sections))) = entries.iter_mut().find(|(key, _)| key == "sections") {
            for section in sections.iter_mut() {
                if let Nbt::Compound(section) = section {
                    changed = changed || section.iter().any(|(key, _)| key == "BlockLight");
                    section.remove_key("BlockLight");
                    changed = changed || section.iter().any(|(key, _)| key == "SkyLight");
                    section.remove_key("SkyLight");
                }
            }
        }
        changed
    }

    pub fn verify_chunk_pos(pos: ChunkPos, upgraded_tag: &mut Nbt) -> bool {
        let int = |key: &str| upgraded_tag.get(key).and_then(Nbt::as_i64).map_or(0, |value| value as i32);
        let stored_pos = ChunkPos::new(int("xPos"), int("zPos"));
        if stored_pos != pos {
            warn!("Chunk {pos} has invalid position {stored_pos}");
        }
        false
    }

    pub fn is_finished(&self) -> bool {
        self.upgrade_progress.is_finished()
    }

    pub fn get_total_chunks(&self) -> i32 {
        self.upgrade_progress.get_total_chunks()
    }

    pub fn get_converted(&self) -> i32 {
        self.upgrade_progress.get_converted()
    }

    pub fn get_skipped(&self) -> i32 {
        self.upgrade_progress.get_skipped()
    }

    pub fn get_status(&self) -> StatusMessage {
        self.status_translator.translate(&self.upgrade_progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached_chunk() -> Nbt {
        let section = |y: i8, cached: bool| {
            let mut fields = vec![("Y".to_string(), Nbt::Byte(y)), ("block_states".to_string(), Nbt::Compound(Vec::new().into()))];
            if cached {
                fields.push(("BlockLight".to_string(), Nbt::ByteArray(vec![0; 2048])));
                fields.push(("SkyLight".to_string(), Nbt::ByteArray(vec![0; 2048])));
            }
            Nbt::Compound(fields.into())
        };
        Nbt::Compound(
            vec![
                ("xPos".to_string(), Nbt::Int(3)),
                ("zPos".to_string(), Nbt::Int(-2)),
                ("Heightmaps".to_string(), Nbt::Compound(Vec::new().into())),
                ("isLightOn".to_string(), Nbt::Byte(1)),
                ("sections".to_string(), Nbt::List(vec![section(0, true), section(1, false)])),
            ]
            .into(),
        )
    }

    #[test]
    fn erasing_the_cache_removes_heightmaps_and_light() {
        let mut tag = cached_chunk();
        assert!(WorldUpgrader::verify_chunk_pos_and_erase_cache(ChunkPos::new(3, -2), &mut tag));
        assert!(tag.get("Heightmaps").is_none());
        assert!(tag.get("isLightOn").is_none());
        let sections = tag.get("sections").unwrap().as_list();
        assert!(sections.iter().all(|section| section.get("BlockLight").is_none() && section.get("SkyLight").is_none()));
        assert!(sections.iter().all(|section| section.get("block_states").is_some()));
        assert!(!WorldUpgrader::verify_chunk_pos_and_erase_cache(ChunkPos::new(3, -2), &mut tag));
    }

    #[test]
    fn verifying_the_position_never_changes_the_chunk() {
        let mut tag = cached_chunk();
        assert!(!WorldUpgrader::verify_chunk_pos(ChunkPos::new(3, -2), &mut tag));
        assert!(!WorldUpgrader::verify_chunk_pos(ChunkPos::new(0, 0), &mut tag));
        assert!(tag.get("Heightmaps").is_some());
    }
}

impl Work {
    fn run(&self) {
        let start = Instant::now();
        let current_version = current_version();
        info!("Upgrading entities");
        self.upgrade_levels(
            DataFixTypes::EntityChunk,
            Builder::new().set_type_and_folder_name("entities").set_recreate_region_files(self.recreate_region_files).track_progress(self.upgrade_progress.clone()),
            |level_specific_builder, _| level_specific_builder,
        );
        info!("Upgrading POIs");
        self.upgrade_levels(
            DataFixTypes::PoiChunk,
            Builder::new().set_type_and_folder_name("poi").set_default_version(1945).set_recreate_region_files(self.recreate_region_files).track_progress(self.upgrade_progress.clone()),
            |level_specific_builder, _| level_specific_builder,
        );
        info!("Upgrading blocks");
        let erase_cache = self.erase_cache;
        self.upgrade_levels(
            DataFixTypes::Chunk,
            Builder::new().set_type("chunk").set_folder_name("region").set_recreate_region_files(self.recreate_region_files).track_progress(self.upgrade_progress.clone()),
            |level_specific_builder, level| {
                let generator_type_name = self.levels.iter().find(|(key, _)| key == level).map(|(_, name)| *name).expect("a known dimension");
                level_specific_builder
                    .set_data_fix_context_tag(Some(WorldUpgrader::get_data_fix_context_tag(level, generator_type_name)))
                    .add_tag_modifier(current_version, if erase_cache { WorldUpgrader::verify_chunk_pos_and_erase_cache } else { WorldUpgrader::verify_chunk_pos })
            },
        );
        let conversion_time = start.elapsed().as_millis() as i64;
        info!("World optimization finished after {} seconds", conversion_time / 1000);
        self.upgrade_progress.set_finished(true);
    }

    fn upgrade_levels(&self, data_fix_type: DataFixTypes, builder: Builder, level_specific_builder: impl Fn(Builder, &str) -> Builder) {
        let mut upgraders: Vec<RegionStorageUpgrader> = Vec::new();
        self.upgrade_progress.reset(data_fix_type);
        self.upgrade_progress.set_type(Type::Regions);
        let builder = builder.set_data_fix_type(data_fix_type);
        let mut previous_copies_file_amounts = 0;
        for (level, _) in &self.levels {
            let mut upgrader = level_specific_builder(builder.copy(), level).build(previous_copies_file_amounts);
            upgrader.init(level, &self.level_directory);
            previous_copies_file_amounts += upgrader.file_amount();
            upgraders.push(upgrader);
        }
        for upgrader in &mut upgraders {
            upgrader.upgrade();
        }
    }
}
