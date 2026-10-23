use std::io;

use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::util::datafix::fixes::references;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;

use super::AutoCloseable;
use super::compressed_nbt::{CompressedNbt, CompressedNbtFile, MissingSeverity};

pub struct PlayerData {
    compressed_nbt: CompressedNbt,
    target_version: i32,
}

impl PlayerData {
    pub fn new(path: CopyOnWriteFSPath, target_version: i32) -> PlayerData {
        PlayerData { compressed_nbt: CompressedNbt::new(path, MissingSeverity::Neutral), target_version }
    }
}

impl CompressedNbtFile for PlayerData {
    fn read(&self) -> io::Result<Option<Dynamic>> {
        Ok(self.compressed_nbt.read_file()?.map(|read_data| {
            let version = read_data.get("DataVersion").as_int(-1);
            get_data_fixer().update(references::PLAYER, read_data, version, self.target_version)
        }))
    }

    fn write(&self, data: &Dynamic) {
        let with_data_version = data.clone().set("DataVersion", data.create_int(self.target_version));
        self.compressed_nbt.write_file(&with_data_version);
    }

    fn path(&self) -> &CopyOnWriteFSPath {
        self.compressed_nbt.path()
    }
}

impl AutoCloseable for PlayerData {
    fn close(&self) -> io::Result<()> {
        Ok(())
    }
}
