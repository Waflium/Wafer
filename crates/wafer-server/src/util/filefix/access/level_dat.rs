use std::io;

use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::util::datafix::fixes::references;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;

use super::AutoCloseable;
use super::compressed_nbt::{CompressedNbt, CompressedNbtFile, MissingSeverity};

pub struct LevelDat {
    compressed_nbt: CompressedNbt,
    target_version: i32,
}

impl LevelDat {
    pub fn new(path: CopyOnWriteFSPath, target_version: i32) -> LevelDat {
        LevelDat { compressed_nbt: CompressedNbt::new(path, MissingSeverity::Important), target_version }
    }
}

impl CompressedNbtFile for LevelDat {
    fn read(&self) -> io::Result<Option<Dynamic>> {
        Ok(self.compressed_nbt.read_file()?.map(|read_data| {
            let content = read_data.get("Data").or_else_empty_map();
            let data_version = content.get("DataVersion").as_int(-1);
            get_data_fixer().update(references::LEVEL, content, data_version, self.target_version)
        }))
    }

    fn write(&self, data: &Dynamic) {
        let with_data_version = data.clone().set("DataVersion", data.create_int(self.target_version));
        self.compressed_nbt.write_file(&data.empty_map().set("Data", with_data_version));
    }

    fn path(&self) -> &CopyOnWriteFSPath {
        self.compressed_nbt.path()
    }
}

impl AutoCloseable for LevelDat {
    fn close(&self) -> io::Result<()> {
        Ok(())
    }
}
