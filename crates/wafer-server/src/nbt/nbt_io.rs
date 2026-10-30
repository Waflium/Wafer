use std::path::Path;

use wafer_protocol::nbt::Nbt;

use crate::util::java_throwable::JavaThrowable;
use crate::world::level::chunk::storage::chunk_stream::{self, Mode, StreamKind};
use crate::world::level::chunk::storage::{decode_nbt, storage_exception};

const READ_COMPRESSED_NEW_INPUT_STREAM: &str = "net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:33)";

pub fn read_compressed(file: &Path) -> Result<Nbt, JavaThrowable> {
    let bytes = std::fs::read(file).map_err(|error| {
        let mut throwable = storage_exception::new_input_stream_failure(error, file);
        throwable.push_frame(READ_COMPRESSED_NEW_INPUT_STREAM);
        throwable
    })?;
    read_compressed_bytes(&bytes)
}

pub fn read_compressed_bytes(bytes: &[u8]) -> Result<Nbt, JavaThrowable> {
    let prefix = chunk_stream::simulate(bytes, StreamKind::Gzip, Mode::ReadCompressed).map_err(|(throwable, _)| throwable)?;
    decode_nbt(&prefix).map_err(|error| JavaThrowable::new(format!("java.io.IOException: {error}"), Vec::new()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    const LEVEL_TAIL: [&str; 4] = [
        "net.minecraft.world.level.storage.LevelStorageSource.readLevelDataTagRaw(LevelStorageSource.java:269)",
        "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTag(LevelStorageSource.java:513)",
        "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:500)",
        "net.minecraft.server.Main.main(Main.java:148)",
    ];

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn trace(bytes: &[u8]) -> String {
        let mut throwable = read_compressed_bytes(bytes).unwrap_err();
        throwable.push_frames(&LEVEL_TAIL);
        throwable.print_stack_trace()
    }

    #[test]
    fn empty_files_fail_in_the_gzip_header() {
        assert_eq!(
            trace(&[]),
            "java.io.EOFException
\tat java.base/java.util.zip.GZIPInputStream.readUByte(GZIPInputStream.java:332)
\tat java.base/java.util.zip.GZIPInputStream.readUShort(GZIPInputStream.java:322)
\tat java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:227)
\tat java.base/java.util.zip.GZIPInputStream.<init>(GZIPInputStream.java:86)
\tat java.base/java.util.zip.GZIPInputStream.<init>(GZIPInputStream.java:118)
\tat net.minecraft.nbt.NbtIo.createDecompressorStream(NbtIo.java:41)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:49)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:36)
\tat net.minecraft.world.level.storage.LevelStorageSource.readLevelDataTagRaw(LevelStorageSource.java:269)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTag(LevelStorageSource.java:513)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:500)
\tat net.minecraft.server.Main.main(Main.java:148)
"
        );
    }

    #[test]
    fn empty_gzip_streams_end_before_the_root_tag() {
        assert_eq!(
            trace(&gzip(&[])),
            "java.io.EOFException
\tat java.base/java.io.DataInputStream.readUnsignedByte(DataInputStream.java:302)
\tat java.base/java.io.DataInputStream.readByte(DataInputStream.java:280)
\tat net.minecraft.nbt.NbtIo.readUnnamedTag(NbtIo.java:177)
\tat net.minecraft.nbt.NbtIo.read(NbtIo.java:109)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:50)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:36)
\tat net.minecraft.world.level.storage.LevelStorageSource.readLevelDataTagRaw(LevelStorageSource.java:269)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTag(LevelStorageSource.java:513)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:500)
\tat net.minecraft.server.Main.main(Main.java:148)
"
        );
    }

    #[test]
    fn non_compound_roots_are_rejected() {
        assert_eq!(
            trace(&gzip(&[3, 0, 0, 0, 0, 0, 1])),
            "java.io.IOException: Root tag must be a named compound tag
\tat net.minecraft.nbt.NbtIo.read(NbtIo.java:113)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:50)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:36)
\tat net.minecraft.world.level.storage.LevelStorageSource.readLevelDataTagRaw(LevelStorageSource.java:269)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTag(LevelStorageSource.java:513)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:500)
\tat net.minecraft.server.Main.main(Main.java:148)
"
        );
    }

    #[test]
    fn invalid_nbt_is_reported_with_the_loading_nbt_data_wrapper() {
        assert_eq!(
            trace(&gzip(b"hello world this is not nbt at all")),
            "net.minecraft.nbt.ReportedNbtException: Loading NBT data
\tat net.minecraft.nbt.NbtIo.readTagSafe(NbtIo.java:195)
\tat net.minecraft.nbt.NbtIo.readUnnamedTag(NbtIo.java:185)
\tat net.minecraft.nbt.NbtIo.read(NbtIo.java:109)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:50)
\tat net.minecraft.nbt.NbtIo.readCompressed(NbtIo.java:36)
\tat net.minecraft.world.level.storage.LevelStorageSource.readLevelDataTagRaw(LevelStorageSource.java:269)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTag(LevelStorageSource.java:513)
\tat net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.getUnfixedDataTagWithFallback(LevelStorageSource.java:500)
\tat net.minecraft.server.Main.main(Main.java:148)
Caused by: java.io.IOException: Invalid tag id: 104
\tat net.minecraft.nbt.TagType$1.createException(TagType.java:54)
\tat net.minecraft.nbt.TagType$1.load(TagType.java:59)
\tat net.minecraft.nbt.TagType$1.load(TagType.java:52)
\tat net.minecraft.nbt.NbtIo.readTagSafe(NbtIo.java:190)
\t... 8 more
"
        );
    }

    #[test]
    fn missing_files_fail_to_open_like_files_new_input_stream() {
        let missing = std::env::temp_dir().join("wafer_nbt_io_missing_level.dat");
        let throwable = read_compressed(&missing).unwrap_err();
        assert!(throwable.description.starts_with("java.nio.file.NoSuchFileException: "));
        assert_eq!(throwable.stack_trace.last(), Some(&READ_COMPRESSED_NEW_INPUT_STREAM));
        assert!(throwable.stack_trace.contains(&"java.base/java.nio.file.Files.newInputStream(Files.java:154)"));
    }
}
