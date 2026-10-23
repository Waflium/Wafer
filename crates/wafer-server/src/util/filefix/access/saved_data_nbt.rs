use std::io;

use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;

use super::AutoCloseable;
use super::compressed_nbt::{CompressedNbt, CompressedNbtFile, MissingSeverity};

pub struct SavedDataNbt {
    compressed_nbt: CompressedNbt,
    type_: TypeReference,
    target_version: i32,
}

impl SavedDataNbt {
    pub fn new(type_: TypeReference, path: CopyOnWriteFSPath, target_version: i32, missing_severity: MissingSeverity) -> SavedDataNbt {
        SavedDataNbt { compressed_nbt: CompressedNbt::new(path, missing_severity), type_, target_version }
    }
}

impl CompressedNbtFile for SavedDataNbt {
    fn read(&self) -> io::Result<Option<Dynamic>> {
        Ok(self.compressed_nbt.read_file()?.map(|read_data| {
            let version = read_data.get("DataVersion").as_int(-1);
            get_data_fixer().update(self.type_, read_data, version, self.target_version).get("data").or_else_empty_map()
        }))
    }

    fn write(&self, data: &Dynamic) {
        let data_tag = data.empty_map().set("data", data.clone());
        let wrapped_and_with_data_version = data_tag.clone().set("DataVersion", data_tag.create_int(self.target_version));
        self.compressed_nbt.write_file(&wrapped_and_with_data_version);
    }

    fn path(&self) -> &CopyOnWriteFSPath {
        self.compressed_nbt.path()
    }
}

impl AutoCloseable for SavedDataNbt {
    fn close(&self) -> io::Result<()> {
        Ok(())
    }
}
