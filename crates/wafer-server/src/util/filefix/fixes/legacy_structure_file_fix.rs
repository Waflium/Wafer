use std::sync::Arc;

use wafer_core::ChunkPos;
use wafer_core::fastutil::Long2ObjectOpenHashMap;
use wafer_core::random::java_string_hash;
use wafer_protocol::nbt::Nbt;

use crate::datafixers::dynamic::{Dynamic, put};
use crate::datafixers::schemas::schema::Schema;
use crate::server::level::chunk_map::get_chunk_data_fix_context_tag;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::datafix::fixes::references;
use crate::util::filefix::access::chunk_nbt::ChunkNbt;
use crate::util::filefix::access::compressed_nbt::{CompressedNbtFile, MissingSeverity};
use crate::util::filefix::access::file_access::FileAccess;
use crate::util::filefix::access::file_access_provider::FileAccessProvider;
use crate::util::filefix::access::file_relation::FileRelation;
use crate::util::filefix::access::file_resource_types::FileResourceTypes;
use crate::util::filefix::access::saved_data_nbt::SavedDataNbt;
use crate::util::filefix::canceled_file_fix_exception::CanceledFileFixException;
use crate::util::filefix::file_fix::FileFix;
use crate::util::filefix::file_fix_exception::FileFixFailure;
use crate::util::java_collections::JavaHashMap;
use crate::util::worldupdate::upgrade_progress::{Status, Type, UpgradeProgress};
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;

pub const STRUCTURE_RANGE: i32 = 8;
pub const OVERWORLD_LEGACY_STRUCTURES: [&str; 5] = ["Monument", "Stronghold", "Mineshaft", "Temple", "Mansion"];
pub const LEGACY_TO_CURRENT_MAP: [(&str, &str); 4] =
    [("Iglu", "Igloo"), ("TeDP", "Desert_Pyramid"), ("TeJP", "Jungle_Pyramid"), ("TeSH", "Swamp_Hut")];
pub const NETHER_LEGACY_STRUCTURES: [&str; 1] = ["Fortress"];
pub const END_LEGACY_STRUCTURES: [&str; 1] = ["EndCity"];
const OVERWORLD_KEY: &str = "minecraft:overworld";
const NETHER_KEY: &str = "minecraft:the_nether";
const END_KEY: &str = "minecraft:the_end";

fn legacy_to_current(id: &str) -> Option<&'static str> {
    LEGACY_TO_CURRENT_MAP.iter().find(|(legacy, _)| *legacy == id).map(|(_, current)| *current)
}

pub struct LegacyStructureFileFix;

impl LegacyStructureFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        file_fix.add_file_content_fix(|files| {
            let overworld_structure_data: Vec<Arc<FileAccess<SavedDataNbt>>> =
                OVERWORLD_LEGACY_STRUCTURES.iter().map(|structure_id| get_legacy_structure_data(files, structure_id)).collect();
            let overworld_info = RegionStorageInfo::new("overworld", OVERWORLD_KEY, "chunk");
            let nether_structure_data: Vec<Arc<FileAccess<SavedDataNbt>>> =
                NETHER_LEGACY_STRUCTURES.iter().map(|structure_id| get_legacy_structure_data(files, structure_id)).collect();
            let nether_info = RegionStorageInfo::new("the_nether", NETHER_KEY, "chunk");
            let end_structure_data: Vec<Arc<FileAccess<SavedDataNbt>>> =
                END_LEGACY_STRUCTURES.iter().map(|structure_id| get_legacy_structure_data(files, structure_id)).collect();
            let end_info = RegionStorageInfo::new("the_end", END_KEY, "chunk");
            let level_dat = files.get_file_access(FileResourceTypes::level_dat(), FileRelation::origin().for_file("level.dat"));
            let overworld_chunks = files.get_file_access(
                FileResourceTypes::chunk(DataFixTypes::Chunk, overworld_info),
                FileRelation::old_overworld().resolve(&FileRelation::region()),
            );
            let nether_chunks = files.get_file_access(
                FileResourceTypes::chunk(DataFixTypes::Chunk, nether_info),
                FileRelation::old_nether().resolve(&FileRelation::region()),
            );
            let end_chunks =
                files.get_file_access(FileResourceTypes::chunk(DataFixTypes::Chunk, end_info), FileRelation::old_end().resolve(&FileRelation::region()));
            Box::new(move |upgrade_progress| {
                let level_data = level_dat.get_only_file().read()?;
                if let Some(level_data) = level_data {
                    upgrade_progress.set_type(Type::LegacyStructures);
                    extract_and_store_legacy_structure_data(
                        &level_data,
                        vec![
                            DimensionFixEntry::new(OVERWORLD_KEY, overworld_structure_data.clone(), overworld_chunks.clone()),
                            DimensionFixEntry::new(NETHER_KEY, nether_structure_data.clone(), nether_chunks.clone()),
                            DimensionFixEntry::new(END_KEY, end_structure_data.clone(), end_chunks.clone()),
                        ],
                        upgrade_progress,
                    )?;
                }
                Ok(())
            })
        });
        file_fix
    }
}

fn extract_and_store_legacy_structure_data(
    level_data: &Dynamic,
    mut dimension_fix_entries: Vec<DimensionFixEntry>,
    upgrade_progress: &UpgradeProgress,
) -> Result<(), FileFixFailure> {
    upgrade_progress.set_status(Status::Counting);
    for dimension_fix_entry in dimension_fix_entries.iter_mut() {
        for structure_data_file_access in &dimension_fix_entry.structure_file_access {
            let target_file = structure_data_file_access.get_only_file();
            if let Some(structure_data) = target_file.read()? {
                extract_legacy_structure_data(&structure_data, &mut dimension_fix_entry.structures);
            }
        }
        upgrade_progress.add_total_file_fix_operations(dimension_fix_entry.structures.len() as i32);
    }
    upgrade_progress.set_status(Status::Upgrading);
    for dimension_fix_entry in &dimension_fix_entries {
        let dimension_key = dimension_fix_entry.dimension_key;
        let chunk_nbt = dimension_fix_entry.chunk_file_access.get_only_file();
        let chunk_generator_type = if dimension_key == OVERWORLD_KEY {
            let generator_name = level_data.get("generatorName").as_string_or("buffet");
            match generator_name.as_str() {
                "flat" => "minecraft:flat",
                "debug_all_block_states" => "minecraft:debug",
                _ => "minecraft:noise",
            }
        } else {
            "minecraft:noise"
        };
        let data_fix_context = get_chunk_data_fix_context_tag(dimension_key, Some(chunk_generator_type));
        store_legacy_structure_data_to_chunks(&dimension_fix_entry.structures, &chunk_nbt, &data_fix_context, upgrade_progress)?;
    }
    Ok(())
}

fn get_legacy_structure_data(files: &Arc<FileAccessProvider>, structure_id: &str) -> Arc<FileAccess<SavedDataNbt>> {
    files.get_file_access(
        FileResourceTypes::saved_data_with_missing_severity(references::SAVED_DATA_STRUCTURE_FEATURE_INDICES, MissingSeverity::Minor),
        FileRelation::data().for_file(&format!("{structure_id}.dat")),
    )
}

fn extract_legacy_structure_data(structure_data: &Dynamic, extracted_data_container: &mut Long2ObjectOpenHashMap<LegacyStructureData>) {
    let features = structure_data.get("Features");
    let map = features.as_map(|key| key, |value| value);
    for (_, mut value) in map {
        let pos = ChunkPos::new(value.get("ChunkX").as_int(0), value.get("ChunkZ").as_int(0)).pack();
        let child_list = value.get("Children").as_list(|child| child);
        if !child_list.is_empty() {
            let id = child_list[0].get("id").as_string().value.and_then(|id| legacy_to_current(&id));
            if let Some(id) = id {
                value = value.clone().set("id", value.create_string(id));
            }
        }
        let final_value = value.clone();
        if let Some(id) = value.get("id").as_string().value {
            extracted_data_container.get_or_insert_with(pos, LegacyStructureData::new).add_start(&id, final_value);
            let origin = ChunkPos::unpack(pos);
            for neighbor_x in origin.x - STRUCTURE_RANGE..=origin.x + STRUCTURE_RANGE {
                for neighbor_z in origin.z - STRUCTURE_RANGE..=origin.z + STRUCTURE_RANGE {
                    extracted_data_container.get_or_insert_with(ChunkPos::new(neighbor_x, neighbor_z).pack(), LegacyStructureData::new).add_index(&id, pos);
                }
            }
        }
    }
}

fn get_compound_or_empty(entries: &mut Vec<(String, Nbt)>, key: &str) -> Vec<(String, Nbt)> {
    match entries.iter().position(|(entry_key, _)| entry_key == key) {
        Some(index) => match &mut entries[index].1 {
            Nbt::Compound(compound) => std::mem::take(compound),
            _ => Vec::new(),
        },
        None => Vec::new(),
    }
}

fn store_legacy_structure_data_to_chunks(
    structures: &Long2ObjectOpenHashMap<LegacyStructureData>,
    chunks_access: &ChunkNbt,
    data_fix_context: &Nbt,
    upgrade_progress: &UpgradeProgress,
) -> Result<(), FileFixFailure> {
    let mut entries: Vec<i64> = structures.stream_keys().collect();
    entries.sort_by_key(|&key| {
        let pos = ChunkPos::unpack(key);
        ChunkPos::new(pos.x >> 5, pos.z >> 5).pack()
    });
    let mut futures = IncrementalFutureSequence::new(8);
    for key in entries {
        if upgrade_progress.is_canceled() {
            return Err(CanceledFileFixException::new().into());
        }
        let legacy_data = structures.get(key).expect("entry");
        chunks_access.update_chunk(ChunkPos::unpack(key), data_fix_context, |mut tag| {
            if !matches!(tag, Nbt::Compound(_)) {
                return tag;
            }
            let Nbt::Compound(tag_entries) = &mut tag else { unreachable!() };
            let mut level_tag = get_compound_or_empty(tag_entries, "Level");
            let mut structure_tag = get_compound_or_empty(&mut level_tag, "Structures");
            let mut start_tag = get_compound_or_empty(&mut structure_tag, "Starts");
            let mut references_tag = get_compound_or_empty(&mut structure_tag, "References");
            for (id, value) in legacy_data.starts.iter() {
                put(&mut start_tag, id, value.value.clone().unwrap_or(Nbt::Compound(Vec::new())));
            }
            for (id, indexes) in legacy_data.indexes.iter() {
                put(&mut references_tag, id, Nbt::LongArray(indexes.clone()));
            }
            put(&mut structure_tag, "Starts", Nbt::Compound(start_tag));
            put(&mut structure_tag, "References", Nbt::Compound(references_tag));
            put(&mut level_tag, "Structures", Nbt::Compound(structure_tag));
            put(tag_entries, "Level", Nbt::Compound(level_tag));
            tag
        })?;
        let finished = futures.push();
        upgrade_progress.increment_finished_operations_by(finished);
    }
    upgrade_progress.increment_finished_operations_by(futures.wait_for_all());
    Ok(())
}

struct DimensionFixEntry {
    dimension_key: &'static str,
    structure_file_access: Vec<Arc<FileAccess<SavedDataNbt>>>,
    chunk_file_access: Arc<FileAccess<ChunkNbt>>,
    structures: Long2ObjectOpenHashMap<LegacyStructureData>,
}

impl DimensionFixEntry {
    fn new(
        dimension_key: &'static str,
        structure_file_access: Vec<Arc<FileAccess<SavedDataNbt>>>,
        chunk_file_access: Arc<FileAccess<ChunkNbt>>,
    ) -> DimensionFixEntry {
        DimensionFixEntry { dimension_key, structure_file_access, chunk_file_access, structures: Long2ObjectOpenHashMap::new() }
    }
}

struct IncrementalFutureSequence {
    max_concurrency: usize,
    futures: usize,
}

impl IncrementalFutureSequence {
    fn new(max_concurrency: usize) -> IncrementalFutureSequence {
        IncrementalFutureSequence { max_concurrency, futures: 0 }
    }

    fn push(&mut self) -> i32 {
        let mut finished = 0;
        if self.futures >= self.max_concurrency {
            finished += self.wait_on_any();
        }
        self.futures += 1;
        finished
    }

    fn wait_on_any(&mut self) -> i32 {
        let old_size = self.futures;
        self.futures = 0;
        old_size as i32
    }

    fn wait_for_all(&mut self) -> i32 {
        let old_size = self.futures;
        self.futures = 0;
        old_size as i32
    }
}

pub struct LegacyStructureData {
    pub starts: JavaHashMap<Dynamic>,
    pub indexes: JavaHashMap<Vec<i64>>,
}

impl LegacyStructureData {
    pub fn new() -> LegacyStructureData {
        LegacyStructureData { starts: JavaHashMap::new(java_string_hash), indexes: JavaHashMap::new(java_string_hash) }
    }

    pub fn starts(&self) -> &JavaHashMap<Dynamic> {
        &self.starts
    }

    pub fn indexes(&self) -> &JavaHashMap<Vec<i64>> {
        &self.indexes
    }

    pub fn add_start(&mut self, id: &str, data: Dynamic) {
        self.starts.insert(id.to_string(), data);
    }

    pub fn add_index(&mut self, id: &str, source_pos: i64) {
        match self.indexes.get_mut(id) {
            Some(indexes) => indexes.push(source_pos),
            None => {
                self.indexes.insert(id.to_string(), vec![source_pos]);
            }
        }
    }
}
