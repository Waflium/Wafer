use crate::datafixers::dsl::TypeReference;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;

use super::chunk_nbt::ChunkNbt;
use super::compressed_nbt::MissingSeverity;
use super::file_resource_type::FileResourceType;
use super::level_dat::LevelDat;
use super::player_data::PlayerData;
use super::saved_data_nbt::SavedDataNbt;

pub struct FileResourceTypes;

impl FileResourceTypes {
    pub fn level_dat() -> FileResourceType<LevelDat> {
        FileResourceType::new(LevelDat::new)
    }

    pub fn player_data() -> FileResourceType<PlayerData> {
        FileResourceType::new(PlayerData::new)
    }

    pub fn saved_data(type_: TypeReference) -> FileResourceType<SavedDataNbt> {
        FileResourceTypes::saved_data_with_missing_severity(type_, MissingSeverity::Neutral)
    }

    pub fn saved_data_with_missing_severity(type_: TypeReference, missing_severity: MissingSeverity) -> FileResourceType<SavedDataNbt> {
        FileResourceType::new(move |path, data_version| SavedDataNbt::new(type_, path, data_version, missing_severity))
    }

    pub fn chunk(type_: DataFixTypes, info: RegionStorageInfo) -> FileResourceType<ChunkNbt> {
        FileResourceType::new(move |path, data_version| ChunkNbt::new(info.clone(), path, type_, data_version))
    }
}
