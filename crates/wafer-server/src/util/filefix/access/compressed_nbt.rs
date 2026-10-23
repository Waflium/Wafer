use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::write::GzEncoder;
use wafer_protocol::nbt::Nbt;

use crate::datafixers::dynamic::Dynamic;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_provider::OpenOptions;
use crate::util::filefix::virtualfilesystem::files;
use crate::world::level::chunk::storage::{decode_compressed_nbt, encode_nbt};

use super::AutoCloseable;

pub fn read_compressed(path: &CopyOnWriteFSPath) -> io::Result<Nbt> {
    let mut input = files::new_input_stream(path)?;
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    decode_compressed_nbt(&bytes).map_err(io::Error::other)
}

pub fn write_compressed(tag: &Nbt, path: &CopyOnWriteFSPath) -> io::Result<()> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&encode_nbt(tag))?;
    let bytes = encoder.finish()?;
    let mut out = files::new_output_stream(path, &OpenOptions::sync_output())?;
    out.write_all(&bytes)?;
    out.sync_data()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MissingSeverity {
    Important,
    Neutral,
    Minor,
}

impl MissingSeverity {
    pub fn log(self, message: &str, path: &CopyOnWriteFSPath) {
        let message = message.replacen("{}", &path.to_string(), 1);
        match self {
            MissingSeverity::Important => error!("{message}"),
            MissingSeverity::Neutral => info!("{message}"),
            MissingSeverity::Minor => {}
        }
    }
}

pub struct CompressedNbt {
    path: CopyOnWriteFSPath,
    missing_severity: MissingSeverity,
}

impl CompressedNbt {
    pub fn new(path: CopyOnWriteFSPath, missing_severity: MissingSeverity) -> CompressedNbt {
        CompressedNbt { path, missing_severity }
    }

    pub fn read_file(&self) -> io::Result<Option<Dynamic>> {
        match read_compressed(&self.path) {
            Ok(nbt) => Ok(Some(Dynamic::new(nbt))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.missing_severity.log("Missing file: {}", &self.path);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub fn write_file(&self, data: &Dynamic) {
        let Some(cast @ Nbt::Compound(_)) = &data.value else {
            panic!("java.lang.ClassCastException: class is not a CompoundTag");
        };
        let result = (|| -> io::Result<()> {
            files::create_directories_safe(&self.path.get_parent().expect("parent"))?;
            write_compressed(cast, &self.path)
        })();
        if let Err(e) = result {
            error!("Failed to write to {}\n{e}", self.path);
        }
    }

    pub fn path(&self) -> &CopyOnWriteFSPath {
        &self.path
    }
}

pub trait CompressedNbtFile: AutoCloseable {
    fn read(&self) -> io::Result<Option<Dynamic>>;

    fn write(&self, data: &Dynamic);

    fn path(&self) -> &CopyOnWriteFSPath;
}
