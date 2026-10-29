use crate::util::java_gzip::GzipInputStream;
use crate::util::java_inflater::{InflaterFailure, InflaterInputStream};
use super::lz4_block_input_stream::Lz4BlockInputStream;
use crate::util::java_throwable::{JavaThrowable, ThrowableReport};

const BUFFER_SIZE: usize = 8192;

const IIS_READ_ZIP: &str = "java.base/java.util.zip.InflaterInputStream.read(InflaterInputStream.java:214)";
const IIS_FILL: &str = "java.base/java.util.zip.InflaterInputStream.fill(InflaterInputStream.java:310)";
const IIS_READ_FILL: &str = "java.base/java.util.zip.InflaterInputStream.read(InflaterInputStream.java:208)";
const IIS_SKIP: &str = "java.base/java.util.zip.InflaterInputStream.skip(InflaterInputStream.java:270)";
const FBIS_FILL: &str = "net.minecraft.util.FastBufferedInputStream.fill(FastBufferedInputStream.java:92)";
const FBIS_READ_SINGLE: &str = "net.minecraft.util.FastBufferedInputStream.read(FastBufferedInputStream.java:30)";
const FBIS_READ_BULK: &str = "net.minecraft.util.FastBufferedInputStream.read(FastBufferedInputStream.java:45)";
const FBIS_READ_DIRECT: &str = "net.minecraft.util.FastBufferedInputStream.read(FastBufferedInputStream.java:43)";
const FBIS_SKIP: &str = "net.minecraft.util.FastBufferedInputStream.skip(FastBufferedInputStream.java:66)";
const DIS_READ_UNSIGNED_BYTE: &str = "java.base/java.io.DataInputStream.readUnsignedByte(DataInputStream.java:300)";
const DIS_READ_UNSIGNED_BYTE_EOF: &str = "java.base/java.io.DataInputStream.readUnsignedByte(DataInputStream.java:302)";
const DIS_READ_BYTE: &str = "java.base/java.io.DataInputStream.readByte(DataInputStream.java:280)";
const DIS_READ_FULLY_READ: &str = "java.base/java.io.DataInputStream.readFully(DataInputStream.java:213)";
const DIS_READ_FULLY_EOF: &str = "java.base/java.io.DataInputStream.readFully(DataInputStream.java:215)";
const DIS_READ_FULLY_ARRAY: &str = "java.base/java.io.DataInputStream.readFully(DataInputStream.java:184)";
const DIS_READ_SHORT: &str = "java.base/java.io.DataInputStream.readShort(DataInputStream.java:324)";
const DIS_READ_UNSIGNED_SHORT: &str = "java.base/java.io.DataInputStream.readUnsignedShort(DataInputStream.java:346)";
const DIS_READ_INT: &str = "java.base/java.io.DataInputStream.readInt(DataInputStream.java:390)";
const DIS_READ_LONG: &str = "java.base/java.io.DataInputStream.readLong(DataInputStream.java:412)";
const DIS_READ_FLOAT: &str = "java.base/java.io.DataInputStream.readFloat(DataInputStream.java:435)";
const DIS_READ_DOUBLE: &str = "java.base/java.io.DataInputStream.readDouble(DataInputStream.java:458)";
const DIS_SKIP_BYTES: &str = "java.base/java.io.DataInputStream.skipBytes(DataInputStream.java:238)";
const DIS_READ_UTF_LENGTH: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:580)";
const DIS_READ_UTF_DATA: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:597)";
const DIS_READ_UTF: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:555)";
const UTF_PARTIAL_2: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:641)";
const UTF_AROUND_2: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:645)";
const UTF_PARTIAL_3: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:654)";
const UTF_AROUND_3: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:659)";
const UTF_DEFAULT: &str = "java.base/java.io.DataInputStream.readUTF(DataInputStream.java:667)";

const NBT_IO_READ_OUTER: &str = "net.minecraft.nbt.NbtIo.read(NbtIo.java:105)";
const NBT_IO_READ_INNER: &str = "net.minecraft.nbt.NbtIo.read(NbtIo.java:109)";
const READ_UNNAMED_TAG_TYPE: &str = "net.minecraft.nbt.NbtIo.readUnnamedTag(NbtIo.java:177)";
const READ_UNNAMED_TAG_SKIP: &str = "net.minecraft.nbt.NbtIo.readUnnamedTag(NbtIo.java:183)";
const READ_UNNAMED_TAG_LOAD: &str = "net.minecraft.nbt.NbtIo.readUnnamedTag(NbtIo.java:185)";
const READ_TAG_SAFE_LOAD: &str = "net.minecraft.nbt.NbtIo.readTagSafe(NbtIo.java:190)";
const READ_TAG_SAFE_THROW: &str = "net.minecraft.nbt.NbtIo.readTagSafe(NbtIo.java:195)";
const NBT_IO_PARSE_TYPE: &str = "net.minecraft.nbt.NbtIo.parse(NbtIo.java:121)";
const NBT_IO_PARSE_SKIP: &str = "net.minecraft.nbt.NbtIo.parse(NbtIo.java:137)";
const NBT_IO_PARSE_PARSE: &str = "net.minecraft.nbt.NbtIo.parse(NbtIo.java:138)";

const REPORTED_NBT_EXCEPTION: &str = "net.minecraft.nbt.ReportedNbtException: Loading NBT data";

const COMPOUND_LOAD_BRIDGE: &str = "net.minecraft.nbt.CompoundTag$1.load(CompoundTag.java:65)";
const COMPOUND_LOAD: &str = "net.minecraft.nbt.CompoundTag$1.load(CompoundTag.java:70)";
const COMPOUND_READ_TYPE: &str = "net.minecraft.nbt.CompoundTag$1.loadCompound(CompoundTag.java:81)";
const COMPOUND_READ_KEY: &str = "net.minecraft.nbt.CompoundTag$1.loadCompound(CompoundTag.java:82)";
const COMPOUND_READ_VALUE: &str = "net.minecraft.nbt.CompoundTag$1.loadCompound(CompoundTag.java:83)";
const COMPOUND_READ_STRING: &str = "net.minecraft.nbt.CompoundTag$1.readString(CompoundTag.java:162)";
const READ_NAMED_TAG_DATA_LOAD: &str = "net.minecraft.nbt.CompoundTag.readNamedTagData(CompoundTag.java:511)";
const READ_NAMED_TAG_DATA_THROW: &str = "net.minecraft.nbt.CompoundTag.readNamedTagData(CompoundTag.java:517)";
const COMPOUND_PARSE: &str = "net.minecraft.nbt.CompoundTag$1.parse(CompoundTag.java:95)";
const COMPOUND_PARSE_TYPE: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:107)";
const COMPOUND_PARSE_SKIP_NAME: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:120)";
const COMPOUND_PARSE_SKIP_VALUE: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:121)";
const COMPOUND_PARSE_KEY: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:126)";
const COMPOUND_PARSE_SKIP_KEYED: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:136)";
const COMPOUND_PARSE_VALUE: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:142)";
const COMPOUND_SKIP_TYPE: &str = "net.minecraft.nbt.CompoundTag$1.skip(CompoundTag.java:173)";
const COMPOUND_SKIP_NAME: &str = "net.minecraft.nbt.CompoundTag$1.skip(CompoundTag.java:174)";
const COMPOUND_SKIP_VALUE: &str = "net.minecraft.nbt.CompoundTag$1.skip(CompoundTag.java:175)";

const LIST_LOAD_BRIDGE: &str = "net.minecraft.nbt.ListTag$1.load(ListTag.java:30)";
const LIST_LOAD: &str = "net.minecraft.nbt.ListTag$1.load(ListTag.java:35)";
const LIST_READ_TYPE: &str = "net.minecraft.nbt.ListTag$1.loadList(ListTag.java:43)";
const LIST_READ_COUNT_CALL: &str = "net.minecraft.nbt.ListTag$1.loadList(ListTag.java:44)";
const LIST_READ_ELEMENT: &str = "net.minecraft.nbt.ListTag$1.loadList(ListTag.java:52)";
const LIST_READ_COUNT: &str = "net.minecraft.nbt.ListTag$1.readListCount(ListTag.java:116)";
const LIST_SKIP_TYPE: &str = "net.minecraft.nbt.ListTag$1.skip(ListTag.java:127)";
const LIST_SKIP_COUNT: &str = "net.minecraft.nbt.ListTag$1.skip(ListTag.java:128)";
const LIST_SKIP_ELEMENTS: &str = "net.minecraft.nbt.ListTag$1.skip(ListTag.java:129)";

const VARIABLE_SIZE_SKIP: &str = "net.minecraft.nbt.TagType$VariableSize.skip(TagType.java:46)";
const STATIC_SIZE_SKIP: &str = "net.minecraft.nbt.TagType$StaticSize.skip(TagType.java:31)";
const STATIC_SIZE_SKIP_COUNT: &str = "net.minecraft.nbt.TagType$StaticSize.skip(TagType.java:36)";
const INVALID_CREATE_EXCEPTION: &str = "net.minecraft.nbt.TagType$1.createException(TagType.java:54)";
const INVALID_LOAD_BRIDGE: &str = "net.minecraft.nbt.TagType$1.load(TagType.java:52)";
const INVALID_LOAD: &str = "net.minecraft.nbt.TagType$1.load(TagType.java:59)";
const INVALID_SKIP_COUNT: &str = "net.minecraft.nbt.TagType$1.skip(TagType.java:69)";
const INVALID_SKIP: &str = "net.minecraft.nbt.TagType$1.skip(TagType.java:74)";

const STRING_SKIP_STRING: &str = "net.minecraft.nbt.StringTag.skipString(StringTag.java:53)";
const STRING_SKIP: &str = "net.minecraft.nbt.StringTag$1.skip(StringTag.java:38)";
const ACCOUNT_BYTES_OUTER: &str = "net.minecraft.nbt.NbtAccounter.accountBytes(NbtAccounter.java:38)";
const ACCOUNT_BYTES_INNER: &str = "net.minecraft.nbt.NbtAccounter.accountBytes(NbtAccounter.java:44)";

struct Primitive {
    load_bridge: &'static str,
    load: &'static str,
    read_accounted: &'static str,
}

const BYTE: Primitive = Primitive {
    load_bridge: "net.minecraft.nbt.ByteTag$1.load(ByteTag.java:25)",
    load: "net.minecraft.nbt.ByteTag$1.load(ByteTag.java:28)",
    read_accounted: "net.minecraft.nbt.ByteTag$1.readAccounted(ByteTag.java:38)",
};
const SHORT: Primitive = Primitive {
    load_bridge: "net.minecraft.nbt.ShortTag$1.load(ShortTag.java:27)",
    load: "net.minecraft.nbt.ShortTag$1.load(ShortTag.java:30)",
    read_accounted: "net.minecraft.nbt.ShortTag$1.readAccounted(ShortTag.java:40)",
};
const INT: Primitive = Primitive {
    load_bridge: "net.minecraft.nbt.IntTag$1.load(IntTag.java:27)",
    load: "net.minecraft.nbt.IntTag$1.load(IntTag.java:30)",
    read_accounted: "net.minecraft.nbt.IntTag$1.readAccounted(IntTag.java:40)",
};
const LONG: Primitive = Primitive {
    load_bridge: "net.minecraft.nbt.LongTag$1.load(LongTag.java:27)",
    load: "net.minecraft.nbt.LongTag$1.load(LongTag.java:30)",
    read_accounted: "net.minecraft.nbt.LongTag$1.readAccounted(LongTag.java:40)",
};
const FLOAT: Primitive = Primitive {
    load_bridge: "net.minecraft.nbt.FloatTag$1.load(FloatTag.java:19)",
    load: "net.minecraft.nbt.FloatTag$1.load(FloatTag.java:22)",
    read_accounted: "net.minecraft.nbt.FloatTag$1.readAccounted(FloatTag.java:32)",
};
const DOUBLE: Primitive = Primitive {
    load_bridge: "net.minecraft.nbt.DoubleTag$1.load(DoubleTag.java:19)",
    load: "net.minecraft.nbt.DoubleTag$1.load(DoubleTag.java:22)",
    read_accounted: "net.minecraft.nbt.DoubleTag$1.readAccounted(DoubleTag.java:32)",
};

struct Array {
    load_bridge: &'static str,
    load: &'static str,
    count_line: &'static str,
    account_line: &'static str,
    new_array_line: &'static str,
    element_line: &'static str,
    skip: &'static str,
    element_size: i32,
}

const BYTE_ARRAY: Array = Array {
    load_bridge: "net.minecraft.nbt.ByteArrayTag$1.load(ByteArrayTag.java:20)",
    load: "net.minecraft.nbt.ByteArrayTag$1.load(ByteArrayTag.java:23)",
    count_line: "net.minecraft.nbt.ByteArrayTag$1.readAccounted(ByteArrayTag.java:33)",
    account_line: "net.minecraft.nbt.ByteArrayTag$1.readAccounted(ByteArrayTag.java:34)",
    new_array_line: "net.minecraft.nbt.ByteArrayTag$1.readAccounted(ByteArrayTag.java:35)",
    element_line: "net.minecraft.nbt.ByteArrayTag$1.readAccounted(ByteArrayTag.java:36)",
    skip: "net.minecraft.nbt.ByteArrayTag$1.skip(ByteArrayTag.java:42)",
    element_size: 1,
};
const INT_ARRAY: Array = Array {
    load_bridge: "net.minecraft.nbt.IntArrayTag$1.load(IntArrayTag.java:20)",
    load: "net.minecraft.nbt.IntArrayTag$1.load(IntArrayTag.java:23)",
    count_line: "net.minecraft.nbt.IntArrayTag$1.readAccounted(IntArrayTag.java:34)",
    account_line: "net.minecraft.nbt.IntArrayTag$1.readAccounted(IntArrayTag.java:35)",
    new_array_line: "net.minecraft.nbt.IntArrayTag$1.readAccounted(IntArrayTag.java:36)",
    element_line: "net.minecraft.nbt.IntArrayTag$1.readAccounted(IntArrayTag.java:38)",
    skip: "net.minecraft.nbt.IntArrayTag$1.skip(IntArrayTag.java:45)",
    element_size: 4,
};
const LONG_ARRAY: Array = Array {
    load_bridge: "net.minecraft.nbt.LongArrayTag$1.load(LongArrayTag.java:20)",
    load: "net.minecraft.nbt.LongArrayTag$1.load(LongArrayTag.java:23)",
    count_line: "net.minecraft.nbt.LongArrayTag$1.readAccounted(LongArrayTag.java:33)",
    account_line: "net.minecraft.nbt.LongArrayTag$1.readAccounted(LongArrayTag.java:34)",
    new_array_line: "net.minecraft.nbt.LongArrayTag$1.readAccounted(LongArrayTag.java:35)",
    element_line: "net.minecraft.nbt.LongArrayTag$1.readAccounted(LongArrayTag.java:37)",
    skip: "net.minecraft.nbt.LongArrayTag$1.skip(LongArrayTag.java:44)",
    element_size: 8,
};

const STRING_LOAD_BRIDGE: &str = "net.minecraft.nbt.StringTag$1.load(StringTag.java:16)";
const STRING_LOAD: &str = "net.minecraft.nbt.StringTag$1.load(StringTag.java:19)";
const STRING_READ_ACCOUNTED: &str = "net.minecraft.nbt.StringTag$1.readAccounted(StringTag.java:31)";

type Failure = JavaThrowable;

fn at(frame: &'static str) -> impl FnOnce(Failure) -> Failure {
    move |mut failure| {
        failure.push_frame(frame);
        failure
    }
}

fn throwable(description: impl Into<String>, frames: &[&'static str]) -> Failure {
    JavaThrowable::new(description, frames.to_vec())
}

fn source_failure(failure: InflaterFailure, skipping: bool) -> Failure {
    match (failure, skipping) {
        (InflaterFailure::ZipException(message), false) => throwable(format!("java.util.zip.ZipException: {message}"), &[IIS_READ_ZIP]),
        (InflaterFailure::ZipException(message), true) => throwable(format!("java.util.zip.ZipException: {message}"), &[IIS_READ_ZIP, IIS_SKIP]),
        (InflaterFailure::Eof, false) => throwable("java.io.EOFException: Unexpected end of ZLIB input stream", &[IIS_FILL, IIS_READ_FILL]),
        (InflaterFailure::Eof, true) => throwable("java.io.EOFException: Unexpected end of ZLIB input stream", &[IIS_FILL, IIS_READ_FILL, IIS_SKIP]),
    }
}

fn eof(frames: &[&'static str]) -> Failure {
    throwable("java.io.EOFException", frames)
}

enum Source {
    Inflater(InflaterInputStream),
    Gzip(GzipInputStream),
    Lz4(Lz4BlockInputStream),
    Plain { data: Vec<u8>, delivered: usize },
}

impl Source {
    fn output(&self) -> &[u8] {
        match self {
            Source::Inflater(stream) => stream.output(),
            Source::Gzip(stream) => stream.output(),
            Source::Lz4(stream) => stream.output(),
            Source::Plain { data, .. } => data,
        }
    }

    fn read(&mut self, length: usize) -> Result<Option<(usize, usize)>, Failure> {
        match self {
            Source::Inflater(stream) => stream.read(length).map_err(|failure| source_failure(failure, false)),
            Source::Gzip(stream) => stream.read(length, false),
            Source::Lz4(stream) => stream.read(length),
            Source::Plain { data, delivered } => {
                if *delivered >= data.len() {
                    return Ok(None);
                }
                let start = *delivered;
                *delivered = (start + length).min(data.len());
                Ok(Some((start, *delivered)))
            }
        }
    }

    fn skip(&mut self, count: usize) -> Result<usize, Failure> {
        match self {
            Source::Inflater(stream) => stream.skip(count).map_err(|failure| source_failure(failure, true)),
            Source::Gzip(stream) => stream.skip(count),
            Source::Lz4(stream) => stream.skip(count),
            Source::Plain { data, delivered } => {
                let skipped = count.min(data.len() - *delivered);
                *delivered += skipped;
                Ok(skipped)
            }
        }
    }
}

struct DataInputStream {
    source: Source,
    position: usize,
    limit: usize,
}

impl DataInputStream {
    fn fill(&mut self) -> Result<(), Failure> {
        match self.source.read(BUFFER_SIZE) {
            Ok(Some((_, end))) => self.limit = end,
            Ok(None) => self.limit = self.position,
            Err(failure) => return Err(at(FBIS_FILL)(failure)),
        }
        Ok(())
    }

    fn fbis_read_single(&mut self) -> Result<Option<u8>, Failure> {
        if self.position >= self.limit {
            self.fill().map_err(at(FBIS_READ_SINGLE))?;
            if self.position >= self.limit {
                return Ok(None);
            }
        }
        self.position += 1;
        Ok(Some(self.source.output()[self.position - 1]))
    }

    fn fbis_read_bulk(&mut self, length: usize) -> Result<Option<usize>, Failure> {
        if self.position >= self.limit {
            if length >= BUFFER_SIZE {
                return match self.source.read(length) {
                    Ok(Some((start, end))) => {
                        self.position = end;
                        self.limit = end;
                        Ok(Some(end - start))
                    }
                    Ok(None) => Ok(None),
                    Err(failure) => Err(at(FBIS_READ_DIRECT)(failure)),
                };
            }
            self.fill().map_err(at(FBIS_READ_BULK))?;
            if self.position >= self.limit {
                return Ok(None);
            }
        }
        let count = length.min(self.limit - self.position);
        self.position += count;
        Ok(Some(count))
    }

    fn fbis_skip(&mut self, count: i32) -> Result<i32, Failure> {
        if count <= 0 {
            return Ok(0);
        }
        let available = self.limit - self.position;
        if available == 0 {
            return match self.source.skip(count as usize) {
                Ok(skipped) => {
                    self.position += skipped;
                    self.limit = self.position;
                    Ok(skipped as i32)
                }
                Err(failure) => Err(at(FBIS_SKIP)(failure)),
            };
        }
        let count = (count as usize).min(available);
        self.position += count;
        Ok(count as i32)
    }

    fn read_byte(&mut self) -> Result<i8, Failure> {
        match self.fbis_read_single() {
            Ok(Some(byte)) => Ok(byte as i8),
            Ok(None) => Err(at(DIS_READ_BYTE)(eof(&[DIS_READ_UNSIGNED_BYTE_EOF]))),
            Err(failure) => Err(at(DIS_READ_BYTE)(at(DIS_READ_UNSIGNED_BYTE)(failure))),
        }
    }

    fn read_fully(&mut self, length: usize) -> Result<(), Failure> {
        let mut total = 0;
        while total < length {
            match self.fbis_read_bulk(length - total) {
                Ok(Some(count)) => total += count,
                Ok(None) => return Err(eof(&[DIS_READ_FULLY_EOF])),
                Err(failure) => return Err(at(DIS_READ_FULLY_READ)(failure)),
            }
        }
        Ok(())
    }

    fn read_primitive(&mut self, length: usize, frame: &'static str) -> Result<&[u8], Failure> {
        self.read_fully(length).map_err(at(frame))?;
        Ok(&self.source.output()[self.position - length..self.position])
    }

    fn read_short(&mut self) -> Result<i16, Failure> {
        let bytes = self.read_primitive(2, DIS_READ_SHORT)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_unsigned_short(&mut self) -> Result<usize, Failure> {
        let bytes = self.read_primitive(2, DIS_READ_UNSIGNED_SHORT)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]) as usize)
    }

    fn read_int(&mut self) -> Result<i32, Failure> {
        let bytes = self.read_primitive(4, DIS_READ_INT)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_long(&mut self) -> Result<i64, Failure> {
        let bytes = self.read_primitive(8, DIS_READ_LONG)?;
        Ok(i64::from_be_bytes(bytes.try_into().expect("eight bytes")))
    }

    fn read_float(&mut self) -> Result<(), Failure> {
        self.read_primitive(4, DIS_READ_FLOAT).map(|_| ())
    }

    fn read_double(&mut self) -> Result<(), Failure> {
        self.read_primitive(8, DIS_READ_DOUBLE).map(|_| ())
    }

    fn skip_bytes(&mut self, count: i32) -> Result<(), Failure> {
        let mut total: i32 = 0;
        while total < count {
            match self.fbis_skip(count - total).map_err(at(DIS_SKIP_BYTES))? {
                skipped if skipped > 0 => total += skipped,
                _ => break,
            }
        }
        Ok(())
    }

    fn read_utf(&mut self) -> Result<String, Failure> {
        let length = self.read_unsigned_short().map_err(at(DIS_READ_UTF_LENGTH)).map_err(at(DIS_READ_UTF))?;
        self.read_fully(length).map_err(at(DIS_READ_UTF_DATA)).map_err(at(DIS_READ_UTF))?;
        let bytes = self.source.output()[self.position - length..self.position].to_vec();
        decode_modified_utf8(&bytes).map_err(at(DIS_READ_UTF))
    }
}

fn decode_modified_utf8(bytes: &[u8]) -> Result<String, Failure> {
    let length = bytes.len();
    let mut count = 0;
    let mut units = Vec::with_capacity(length);
    let malformed = |message: String, frame: &'static str| throwable(format!("java.io.UTFDataFormatException: {message}"), &[frame]);
    while count < length {
        let c = bytes[count] as u16;
        match c >> 4 {
            0..=7 => {
                count += 1;
                units.push(c);
            }
            12 | 13 => {
                count += 2;
                if count > length {
                    return Err(malformed("malformed input: partial character at end".to_string(), UTF_PARTIAL_2));
                }
                let second = bytes[count - 1] as u16;
                if second & 0xc0 != 0x80 {
                    return Err(malformed(format!("malformed input around byte {count}"), UTF_AROUND_2));
                }
                units.push(((c & 0x1f) << 6) | (second & 0x3f));
            }
            14 => {
                count += 3;
                if count > length {
                    return Err(malformed("malformed input: partial character at end".to_string(), UTF_PARTIAL_3));
                }
                let (second, third) = (bytes[count - 2] as u16, bytes[count - 1] as u16);
                if second & 0xc0 != 0x80 || third & 0xc0 != 0x80 {
                    return Err(malformed(format!("malformed input around byte {}", count - 1), UTF_AROUND_3));
                }
                units.push(((c & 0x0f) << 12) | ((second & 0x3f) << 6) | (third & 0x3f));
            }
            _ => return Err(malformed(format!("malformed input around byte {count}"), UTF_DEFAULT)),
        }
    }
    Ok(String::from_utf16_lossy(&units))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind {
    End,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    ByteArray,
    String,
    List,
    Compound,
    IntArray,
    LongArray,
    Invalid(i8),
}

fn tag_kind(id: i8) -> TagKind {
    match id {
        0 => TagKind::End,
        1 => TagKind::Byte,
        2 => TagKind::Short,
        3 => TagKind::Int,
        4 => TagKind::Long,
        5 => TagKind::Float,
        6 => TagKind::Double,
        7 => TagKind::ByteArray,
        8 => TagKind::String,
        9 => TagKind::List,
        10 => TagKind::Compound,
        11 => TagKind::IntArray,
        12 => TagKind::LongArray,
        other => TagKind::Invalid(other),
    }
}

fn invalid_tag_id(id: i8, frames: &[&'static str]) -> Failure {
    throwable(format!("java.io.IOException: Invalid tag id: {id}"), frames)
}

fn negative_size(bytes_per_entry: i64, count: i32, caller: &'static str) -> Failure {
    throwable(
        format!("java.lang.IllegalArgumentException: Tried to account NBT tag with negative size: {}", bytes_per_entry * count as i64),
        &[ACCOUNT_BYTES_INNER, ACCOUNT_BYTES_OUTER, caller],
    )
}

struct Parser {
    input: DataInputStream,
    depth: u32,
    live_bytes: u64,
}

const MAX_DEPTH: u32 = 512;

fn depth_exceeded() -> Failure {
    throwable(format!("net.minecraft.nbt.NbtAccounterException: Tried to read NBT tag with too high complexity, depth > {MAX_DEPTH}"), &[])
}

impl Parser {
    fn push_depth(&mut self) -> Result<(), Failure> {
        if self.depth >= MAX_DEPTH {
            return Err(depth_exceeded());
        }
        self.depth += 1;
        Ok(())
    }

    fn load(&mut self, kind: TagKind) -> Result<(), Failure> {
        match kind {
            TagKind::End => Ok(()),
            TagKind::Byte => self.load_byte().map_err(at(BYTE.read_accounted)).map_err(at(BYTE.load)).map_err(at(BYTE.load_bridge)),
            TagKind::Short => self.input.read_short().map(|_| ()).map_err(at(SHORT.read_accounted)).map_err(at(SHORT.load)).map_err(at(SHORT.load_bridge)),
            TagKind::Int => self.input.read_int().map(|_| ()).map_err(at(INT.read_accounted)).map_err(at(INT.load)).map_err(at(INT.load_bridge)),
            TagKind::Long => self.input.read_long().map(|_| ()).map_err(at(LONG.read_accounted)).map_err(at(LONG.load)).map_err(at(LONG.load_bridge)),
            TagKind::Float => self.input.read_float().map_err(at(FLOAT.read_accounted)).map_err(at(FLOAT.load)).map_err(at(FLOAT.load_bridge)),
            TagKind::Double => self.input.read_double().map_err(at(DOUBLE.read_accounted)).map_err(at(DOUBLE.load)).map_err(at(DOUBLE.load_bridge)),
            TagKind::ByteArray => self.load_array(&BYTE_ARRAY),
            TagKind::IntArray => self.load_array(&INT_ARRAY),
            TagKind::LongArray => self.load_array(&LONG_ARRAY),
            TagKind::String => self.input.read_utf().map(|_| ()).map_err(at(STRING_READ_ACCOUNTED)).map_err(at(STRING_LOAD)).map_err(at(STRING_LOAD_BRIDGE)),
            TagKind::List => self.load_list(),
            TagKind::Compound => self.load_compound(),
            TagKind::Invalid(id) => Err(invalid_tag_id(id, &[INVALID_CREATE_EXCEPTION, INVALID_LOAD, INVALID_LOAD_BRIDGE])),
        }
    }

    fn load_byte(&mut self) -> Result<(), Failure> {
        self.input.read_byte().map(|_| ())
    }

    fn load_array(&mut self, array: &Array) -> Result<(), Failure> {
        self.load_array_accounted(array).map_err(at(array.load)).map_err(at(array.load_bridge))
    }

    fn load_array_accounted(&mut self, array: &Array) -> Result<(), Failure> {
        let length = self.input.read_int().map_err(at(array.count_line))?;
        if length < 0 {
            return Err(negative_size(array.element_size as i64, length, array.account_line));
        }
        if let Some(error) = crate::util::java_heap::array_allocation_failure(length as i64, array.element_size as i64, self.live_bytes) {
            return Err(throwable(error, &[array.new_array_line]));
        }
        self.live_bytes += length as u64 * array.element_size as u64;
        let length = length as usize;
        if array.element_size == 1 {
            return self.input.read_fully(length).map_err(at(DIS_READ_FULLY_ARRAY)).map_err(at(array.element_line));
        }
        for _ in 0..length {
            match array.element_size {
                4 => self.input.read_int().map(|_| ()),
                _ => self.input.read_long().map(|_| ()),
            }
            .map_err(at(array.element_line))?;
        }
        Ok(())
    }

    fn read_list_count(&mut self) -> Result<i32, Failure> {
        let count = self.input.read_int().map_err(at(LIST_READ_COUNT))?;
        if count < 0 {
            return Err(throwable(format!("net.minecraft.nbt.NbtFormatException: ListTag length cannot be negative: {count}"), &[LIST_READ_COUNT_NEGATIVE]));
        }
        Ok(count)
    }

    fn load_list(&mut self) -> Result<(), Failure> {
        self.push_depth().map_err(at(LIST_LOAD))?;
        let result = self.load_list_body().map_err(at(LIST_LOAD)).map_err(at(LIST_LOAD_BRIDGE));
        self.depth -= 1;
        result
    }

    fn load_list_body(&mut self) -> Result<(), Failure> {
        let type_id = self.input.read_byte().map_err(at(LIST_READ_TYPE))?;
        let count = self.read_list_count().map_err(at(LIST_READ_COUNT_CALL))?;
        if type_id == 0 && count > 0 {
            return Err(throwable("net.minecraft.nbt.NbtFormatException: Missing type on ListTag", &[LIST_MISSING_TYPE]));
        }
        if let Some(error) = crate::util::java_heap::array_allocation_failure(count as i64, 4, self.live_bytes) {
            return Err(throwable(error, &[ARRAY_LIST_INIT, LIST_NEW_ARRAY_LIST]));
        }
        self.live_bytes += count as u64 * 4;
        let kind = tag_kind(type_id);
        for _ in 0..count {
            self.load(kind).map_err(at(LIST_READ_ELEMENT))?;
        }
        Ok(())
    }

    fn load_compound(&mut self) -> Result<(), Failure> {
        self.push_depth().map_err(at(COMPOUND_LOAD))?;
        let result = self.load_compound_body().map_err(at(COMPOUND_LOAD)).map_err(at(COMPOUND_LOAD_BRIDGE));
        self.depth -= 1;
        result
    }

    fn load_compound_body(&mut self) -> Result<(), Failure> {
        loop {
            let type_id = self.input.read_byte().map_err(at(COMPOUND_READ_TYPE))?;
            if type_id == 0 {
                return Ok(());
            }
            let key = self.input.read_utf().map_err(at(COMPOUND_READ_STRING)).map_err(at(COMPOUND_READ_KEY))?;
            self.read_named_tag_data(tag_kind(type_id), key).map_err(at(COMPOUND_READ_VALUE))?;
        }
    }

    fn read_named_tag_data(&mut self, kind: TagKind, name: String) -> Result<(), Failure> {
        match self.load(kind).map_err(at(READ_NAMED_TAG_DATA_LOAD)) {
            Err(failure) if failure.is_io_exception() => {
                let details = vec![("Tag name".to_string(), name), ("Tag type".to_string(), tag_type_name(kind))];
                Err(wrap(failure, READ_NAMED_TAG_DATA_THROW, details, READ_NAMED_TAG_DATA_LOAD))
            }
            other => other,
        }
    }

    fn skip_string(&mut self) -> Result<(), Failure> {
        let length = self.input.read_unsigned_short().map_err(at(STRING_SKIP_STRING))?;
        self.input.skip_bytes(length as i32).map_err(at(STRING_SKIP_STRING))
    }

    fn skip(&mut self, kind: TagKind) -> Result<(), Failure> {
        match kind {
            TagKind::End => Ok(()),
            TagKind::Byte | TagKind::Short | TagKind::Int | TagKind::Long | TagKind::Float | TagKind::Double => {
                self.input.skip_bytes(static_size(kind)).map_err(at(STATIC_SIZE_SKIP))
            }
            TagKind::ByteArray => self.skip_array(&BYTE_ARRAY),
            TagKind::IntArray => self.skip_array(&INT_ARRAY),
            TagKind::LongArray => self.skip_array(&LONG_ARRAY),
            TagKind::String => self.skip_string().map_err(at(STRING_SKIP)),
            TagKind::List => {
                self.push_depth().map_err(at(LIST_SKIP_TYPE))?;
                let result = self.skip_list_body();
                self.depth -= 1;
                result
            }
            TagKind::Compound => {
                self.push_depth().map_err(at(COMPOUND_SKIP_TYPE))?;
                let result = self.skip_compound_body();
                self.depth -= 1;
                result
            }
            TagKind::Invalid(id) => Err(invalid_tag_id(id, &[INVALID_CREATE_EXCEPTION, INVALID_SKIP])),
        }
    }

    fn skip_array(&mut self, array: &Array) -> Result<(), Failure> {
        let length = self.input.read_int().map_err(at(array.skip))?;
        self.input.skip_bytes(length.wrapping_mul(array.element_size)).map_err(at(array.skip))
    }

    fn skip_list_body(&mut self) -> Result<(), Failure> {
        let type_id = self.input.read_byte().map_err(at(LIST_SKIP_TYPE))?;
        let kind = tag_kind(type_id);
        let count = self.input.read_int().map_err(at(LIST_SKIP_COUNT))?;
        self.skip_count(kind, count).map_err(at(LIST_SKIP_ELEMENTS))
    }

    fn skip_count(&mut self, kind: TagKind, count: i32) -> Result<(), Failure> {
        match kind {
            TagKind::Byte | TagKind::Short | TagKind::Int | TagKind::Long | TagKind::Float | TagKind::Double => {
                self.input.skip_bytes(static_size(kind).wrapping_mul(count)).map_err(at(STATIC_SIZE_SKIP_COUNT))
            }
            TagKind::End => Ok(()),
            TagKind::Invalid(id) => Err(invalid_tag_id(id, &[INVALID_CREATE_EXCEPTION, INVALID_SKIP_COUNT])),
            _ => {
                for _ in 0..count.max(0) {
                    self.skip(kind).map_err(at(VARIABLE_SIZE_SKIP))?;
                }
                Ok(())
            }
        }
    }

    fn skip_compound_body(&mut self) -> Result<(), Failure> {
        loop {
            let type_id = self.input.read_byte().map_err(at(COMPOUND_SKIP_TYPE))?;
            if type_id == 0 {
                return Ok(());
            }
            self.skip_string().map_err(at(COMPOUND_SKIP_NAME))?;
            self.skip(tag_kind(type_id)).map_err(at(COMPOUND_SKIP_VALUE))?;
        }
    }
}

const LIST_READ_COUNT_NEGATIVE: &str = "net.minecraft.nbt.ListTag$1.readListCount(ListTag.java:118)";
const ARRAY_LIST_INIT: &str = "java.base/java.util.ArrayList.<init>(ArrayList.java:157)";
const LIST_NEW_ARRAY_LIST: &str = "net.minecraft.nbt.ListTag$1.loadList(ListTag.java:50)";
const LIST_MISSING_TYPE: &str ="net.minecraft.nbt.ListTag$1.loadList(ListTag.java:46)";

fn static_size(kind: TagKind) -> i32 {
    match kind {
        TagKind::Byte => 1,
        TagKind::Short => 2,
        TagKind::Int | TagKind::Float => 4,
        _ => 8,
    }
}

fn wrap(cause: Failure, throw_frame: &'static str, details: Vec<(String, String)>, load_frame: &'static str) -> Failure {
    let report = ThrowableReport { title: "Loading NBT data".to_string(), category: "NBT Tag".to_string(), details, category_stack_trace: vec![load_frame] };
    JavaThrowable::new(REPORTED_NBT_EXCEPTION, vec![throw_frame]).with_cause(cause).with_report(report)
}

fn tag_type_name(kind: TagKind) -> String {
    match kind {
        TagKind::End => "END".to_string(),
        TagKind::Byte => "BYTE".to_string(),
        TagKind::Short => "SHORT".to_string(),
        TagKind::Int => "INT".to_string(),
        TagKind::Long => "LONG".to_string(),
        TagKind::Float => "FLOAT".to_string(),
        TagKind::Double => "DOUBLE".to_string(),
        TagKind::ByteArray => "BYTE[]".to_string(),
        TagKind::String => "STRING".to_string(),
        TagKind::List => "LIST".to_string(),
        TagKind::Compound => "COMPOUND".to_string(),
        TagKind::IntArray => "INT[]".to_string(),
        TagKind::LongArray => "LONG[]".to_string(),
        TagKind::Invalid(id) => format!("INVALID[{id}]"),
    }
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScanKind {
    OldData,
    Structures,
}

pub enum Mode {
    Read,
    Scan(ScanKind),
}

#[derive(Clone, Copy)]
pub enum StreamKind {
    Gzip,
    Deflate,
    None,
    Lz4,
}

const WRAP_FRAMES: [&str; 4] = [
    "net.minecraft.world.level.chunk.storage.RegionFileVersion.lambda$static$0(RegionFileVersion.java:30)",
    "net.minecraft.world.level.chunk.storage.RegionFileVersion.wrap(RegionFileVersion.java:115)",
    "net.minecraft.world.level.chunk.storage.RegionFile.createChunkInputStream(RegionFile.java:262)",
    "net.minecraft.world.level.chunk.storage.RegionFile.getChunkDataInputStream(RegionFile.java:228)",
];

pub fn simulate(compressed: &[u8], kind: StreamKind, mode: Mode) -> Result<Vec<u8>, (JavaThrowable, bool)> {
    let source = match kind {
        StreamKind::Deflate => Source::Inflater(InflaterInputStream::new(compressed)),
        StreamKind::None => Source::Plain { data: compressed.to_vec(), delivered: 0 },
        StreamKind::Lz4 => Source::Lz4(Lz4BlockInputStream::new(compressed)),
        StreamKind::Gzip => match GzipInputStream::new(compressed) {
            Ok(stream) => Source::Gzip(stream),
            Err(mut failure) => {
                failure.push_frames(&WRAP_FRAMES);
                return Err((failure, true));
            }
        },
    };
    let mut parser = Parser { input: DataInputStream { source, position: 0, limit: 0 }, depth: 0, live_bytes: 0 };
    let result = match mode {
        Mode::Read => read_root(&mut parser),
        Mode::Scan(kind) => scan_root(&mut parser, kind),
    };
    result.map(|()| parser.input.source.output()[..parser.input.position].to_vec()).map_err(|failure| (failure, false))
}

fn read_root(parser: &mut Parser) -> Result<(), Failure> {
    read_unnamed_tag(parser).map_err(at(NBT_IO_READ_INNER)).map_err(at(NBT_IO_READ_OUTER))
}

fn read_unnamed_tag(parser: &mut Parser) -> Result<(), Failure> {
    let type_id = parser.input.read_byte().map_err(at(READ_UNNAMED_TAG_TYPE))?;
    if type_id == 0 {
        return Ok(());
    }
    parser.skip_string().map_err(at(READ_UNNAMED_TAG_SKIP))?;
    let kind = tag_kind(type_id);
    match parser.load(kind).map_err(at(READ_TAG_SAFE_LOAD)) {
        Err(failure) if failure.is_io_exception() => Err(wrap(failure, READ_TAG_SAFE_THROW, vec![("Tag type".to_string(), type_id.to_string())], READ_TAG_SAFE_LOAD)),
        other => other,
    }
    .map_err(at(READ_UNNAMED_TAG_LOAD))
}

struct FieldTree {
    depth: usize,
    selected_fields: Vec<(&'static str, TagKind)>,
    fields_to_recurse: Vec<(&'static str, usize)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryResult {
    Enter,
    Skip,
    Break,
}

struct CollectFields {
    fields_to_get_count: i32,
    wanted_types: Vec<TagKind>,
    trees: Vec<FieldTree>,
    stack: Vec<usize>,
    container_stack_size: usize,
}

impl CollectFields {
    fn new(kind: ScanKind) -> CollectFields {
        let mut trees = vec![FieldTree { depth: 1, selected_fields: Vec::new(), fields_to_recurse: Vec::new() }];
        let mut wanted_types = vec![TagKind::Compound];
        let mut count = 0;
        let add_entry = |trees: &mut Vec<FieldTree>, path: &[&'static str], name: &'static str, tag_kind: TagKind| {
            let mut current = 0;
            for key in path {
                current = match trees[current].fields_to_recurse.iter().find(|(existing, _)| existing == key) {
                    Some((_, index)) => *index,
                    None => {
                        let depth = trees[current].depth + 1;
                        trees.push(FieldTree { depth, selected_fields: Vec::new(), fields_to_recurse: Vec::new() });
                        let index = trees.len() - 1;
                        trees[current].fields_to_recurse.push((key, index));
                        index
                    }
                };
            }
            trees[current].selected_fields.push((name, tag_kind));
        };
        let selectors: &[(&[&'static str], &'static str, TagKind)] = match kind {
            ScanKind::OldData => &[(&[], "DataVersion", TagKind::Int), (&[], "blending_data", TagKind::Compound)],
            ScanKind::Structures => &[(&[], "DataVersion", TagKind::Int), (&["Level", "Structures"], "Starts", TagKind::Compound), (&["structures"], "starts", TagKind::Compound)],
        };
        for (path, name, tag_kind) in selectors {
            add_entry(&mut trees, path, name, *tag_kind);
            count += 1;
            if !wanted_types.contains(tag_kind) {
                wanted_types.push(*tag_kind);
            }
        }
        CollectFields { fields_to_get_count: count, wanted_types, trees, stack: vec![0], container_stack_size: 1 }
    }

    fn depth(&self) -> usize {
        self.container_stack_size - 1
    }

    fn enter_container_if_needed(&mut self, kind: TagKind) {
        if matches!(kind, TagKind::List | TagKind::Compound) {
            self.container_stack_size += 1;
        }
    }

    fn visit_root_entry(&mut self, kind: TagKind) -> bool {
        if kind != TagKind::Compound {
            return false;
        }
        self.enter_container_if_needed(kind);
        true
    }

    fn visit_entry(&mut self, kind: TagKind) -> EntryResult {
        let frame = &self.trees[*self.stack.last().expect("frame")];
        if self.depth() > frame.depth {
            EntryResult::Enter
        } else if self.fields_to_get_count <= 0 {
            EntryResult::Break
        } else if !self.wanted_types.contains(&kind) {
            EntryResult::Skip
        } else {
            EntryResult::Enter
        }
    }

    fn visit_entry_with_key(&mut self, kind: TagKind, key: &[u8]) -> EntryResult {
        let frame_index = *self.stack.last().expect("frame");
        if self.depth() > self.trees[frame_index].depth {
            self.enter_container_if_needed(kind);
            return EntryResult::Enter;
        }
        let selected = self.trees[frame_index].selected_fields.iter().position(|(name, selected_kind)| name.as_bytes() == key && *selected_kind == kind);
        if let Some(index) = selected {
            self.trees[frame_index].selected_fields.remove(index);
            self.fields_to_get_count -= 1;
            self.enter_container_if_needed(kind);
            return EntryResult::Enter;
        }
        if kind == TagKind::Compound
            && let Some((_, index)) = self.trees[frame_index].fields_to_recurse.iter().find(|(name, _)| name.as_bytes() == key)
        {
            self.stack.push(*index);
            self.enter_container_if_needed(kind);
            return EntryResult::Enter;
        }
        EntryResult::Skip
    }

    fn visit_container_end(&mut self) {
        if self.depth() == self.trees[*self.stack.last().expect("frame")].depth {
            self.stack.pop();
        }
        self.container_stack_size -= 1;
    }
}

const COMPOUND_PARSE_BREAK_NAME: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:115)";
const COMPOUND_PARSE_BREAK_VALUE: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:116)";
const COMPOUND_PARSE_TAIL_TYPE: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:152)";
const COMPOUND_PARSE_TAIL_NAME: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:153)";
const COMPOUND_PARSE_TAIL_VALUE: &str = "net.minecraft.nbt.CompoundTag$1.parseCompound(CompoundTag.java:154)";
const LIST_PARSE: &str = "net.minecraft.nbt.ListTag$1.parse(ListTag.java:61)";
const LIST_PARSE_TYPE: &str = "net.minecraft.nbt.ListTag$1.parseList(ListTag.java:69)";
const LIST_PARSE_COUNT: &str = "net.minecraft.nbt.ListTag$1.parseList(ListTag.java:70)";
const LIST_PARSE_ELEMENT: &str = "net.minecraft.nbt.ListTag$1.parseList(ListTag.java:99)";


const BYTE_PARSE: &str = "net.minecraft.nbt.ByteTag$1.parse(ByteTag.java:33)";
const SHORT_PARSE: &str = "net.minecraft.nbt.ShortTag$1.parse(ShortTag.java:35)";
const INT_PARSE_LINE: &str = "net.minecraft.nbt.IntTag$1.parse(IntTag.java:35)";
const LONG_PARSE: &str = "net.minecraft.nbt.LongTag$1.parse(LongTag.java:35)";
const FLOAT_PARSE: &str = "net.minecraft.nbt.FloatTag$1.parse(FloatTag.java:27)";
const DOUBLE_PARSE: &str = "net.minecraft.nbt.DoubleTag$1.parse(DoubleTag.java:27)";
const STRING_PARSE: &str = "net.minecraft.nbt.StringTag$1.parse(StringTag.java:24)";
const BYTE_ARRAY_PARSE: &str = "net.minecraft.nbt.ByteArrayTag$1.parse(ByteArrayTag.java:28)";
const INT_ARRAY_PARSE: &str = "net.minecraft.nbt.IntArrayTag$1.parse(IntArrayTag.java:28)";
const LONG_ARRAY_PARSE: &str = "net.minecraft.nbt.LongArrayTag$1.parse(LongArrayTag.java:28)";

impl Parser {
    fn parse_value(&mut self, kind: TagKind, visitor: &mut CollectFields) -> Result<bool, Failure> {
        match kind {
            TagKind::End => Ok(true),
            TagKind::Byte => self.input.read_byte().map(|_| ()).map_err(at(BYTE.read_accounted)).map_err(at(BYTE_PARSE)).map(|()| true),
            TagKind::Short => self.input.read_short().map(|_| ()).map_err(at(SHORT.read_accounted)).map_err(at(SHORT_PARSE)).map(|()| true),
            TagKind::Int => self.input.read_int().map(|_| ()).map_err(at(INT.read_accounted)).map_err(at(INT_PARSE_LINE)).map(|()| true),
            TagKind::Long => self.input.read_long().map(|_| ()).map_err(at(LONG.read_accounted)).map_err(at(LONG_PARSE)).map(|()| true),
            TagKind::Float => self.input.read_float().map_err(at(FLOAT.read_accounted)).map_err(at(FLOAT_PARSE)).map(|()| true),
            TagKind::Double => self.input.read_double().map_err(at(DOUBLE.read_accounted)).map_err(at(DOUBLE_PARSE)).map(|()| true),
            TagKind::String => self.input.read_utf().map_err(at(STRING_READ_ACCOUNTED)).map_err(at(STRING_PARSE)).map(|_| true),
            TagKind::ByteArray => self.load_array_accounted(&BYTE_ARRAY).map_err(at(BYTE_ARRAY_PARSE)).map(|()| true),
            TagKind::IntArray => self.load_array_accounted(&INT_ARRAY).map_err(at(INT_ARRAY_PARSE)).map(|()| true),
            TagKind::LongArray => self.load_array_accounted(&LONG_ARRAY).map_err(at(LONG_ARRAY_PARSE)).map(|()| true),
            TagKind::List => self.parse_list(visitor),
            TagKind::Compound => self.parse_compound(visitor),
            TagKind::Invalid(id) => Err(invalid_tag_id(id, &[INVALID_CREATE_EXCEPTION, INVALID_LOAD, INVALID_LOAD_BRIDGE])),
        }
    }

    fn parse_list(&mut self, visitor: &mut CollectFields) -> Result<bool, Failure> {
        self.push_depth().map_err(at(LIST_PARSE))?;
        let result = self.parse_list_body(visitor).map_err(at(LIST_PARSE));
        self.depth -= 1;
        result
    }

    fn parse_list_body(&mut self, visitor: &mut CollectFields) -> Result<bool, Failure> {
        let type_id = self.input.read_byte().map_err(at(LIST_PARSE_TYPE))?;
        let element_kind = tag_kind(type_id);
        let count = self.read_list_count().map_err(at(LIST_PARSE_COUNT))?;
        for _ in 0..count {
            visitor.enter_container_if_needed(element_kind);
            self.parse_value(element_kind, visitor).map_err(at(LIST_PARSE_ELEMENT))?;
        }
        visitor.visit_container_end();
        Ok(true)
    }

    fn parse_compound(&mut self, visitor: &mut CollectFields) -> Result<bool, Failure> {
        self.push_depth().map_err(at(COMPOUND_PARSE))?;
        let result = self.parse_compound_body(visitor).map_err(at(COMPOUND_PARSE));
        self.depth -= 1;
        result
    }

    fn parse_compound_body(&mut self, visitor: &mut CollectFields) -> Result<bool, Failure> {
        let mut broke = false;
        loop {
            let type_id = self.input.read_byte().map_err(at(COMPOUND_PARSE_TYPE))?;
            if type_id == 0 {
                break;
            }
            let kind = tag_kind(type_id);
            match visitor.visit_entry(kind) {
                EntryResult::Break => {
                    self.skip_string().map_err(at(COMPOUND_PARSE_BREAK_NAME))?;
                    self.skip(kind).map_err(at(COMPOUND_PARSE_BREAK_VALUE))?;
                    broke = true;
                    break;
                }
                EntryResult::Skip => {
                    self.skip_string().map_err(at(COMPOUND_PARSE_SKIP_NAME))?;
                    self.skip(kind).map_err(at(COMPOUND_PARSE_SKIP_VALUE))?;
                    continue;
                }
                EntryResult::Enter => {}
            }
            let key = read_key(self).map_err(at(COMPOUND_PARSE_KEY))?;
            match visitor.visit_entry_with_key(kind, &key) {
                EntryResult::Break => {
                    self.skip(kind).map_err(at(COMPOUND_PARSE_SKIP_KEYED))?;
                    broke = true;
                    break;
                }
                EntryResult::Skip => {
                    self.skip(kind).map_err(at(COMPOUND_PARSE_SKIP_KEYED))?;
                    continue;
                }
                EntryResult::Enter => {}
            }
            if !self.parse_value(kind, visitor).map_err(at(COMPOUND_PARSE_VALUE))? {
                return Ok(false);
            }
        }
        if broke {
            loop {
                let type_id = self.input.read_byte().map_err(at(COMPOUND_PARSE_TAIL_TYPE))?;
                if type_id == 0 {
                    break;
                }
                self.skip_string().map_err(at(COMPOUND_PARSE_TAIL_NAME))?;
                self.skip(tag_kind(type_id)).map_err(at(COMPOUND_PARSE_TAIL_VALUE))?;
            }
        }
        visitor.visit_container_end();
        Ok(true)
    }
}

fn scan_root(parser: &mut Parser, kind: ScanKind) -> Result<(), Failure> {
    let mut visitor = CollectFields::new(kind);
    let type_id = parser.input.read_byte().map_err(at(NBT_IO_PARSE_TYPE))?;
    let root_kind = tag_kind(type_id);
    if root_kind == TagKind::End || !visitor.visit_root_entry(root_kind) {
        return Ok(());
    }
    parser.skip_string().map_err(at(NBT_IO_PARSE_SKIP))?;
    parser.parse_value(root_kind, &mut visitor).map(|_| ()).map_err(at(NBT_IO_PARSE_PARSE))
}

fn read_key(parser: &mut Parser) -> Result<Vec<u8>, Failure> {
    let length = parser.input.read_unsigned_short().map_err(at(DIS_READ_UTF_LENGTH)).map_err(at(DIS_READ_UTF)).map_err(at(COMPOUND_READ_STRING))?;
    parser.input.read_fully(length).map_err(at(DIS_READ_UTF_DATA)).map_err(at(DIS_READ_UTF)).map_err(at(COMPOUND_READ_STRING))?;
    let key = parser.input.source.output()[parser.input.position - length..parser.input.position].to_vec();
    decode_modified_utf8(&key).map_err(at(DIS_READ_UTF)).map_err(at(COMPOUND_READ_STRING))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    const CASES: &str = include_str!("chunk_stream_cases.txt");

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len() / 2).map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap()).collect()
    }

    fn render(failure: &JavaThrowable, stop: &str) -> String {
        let mut out = String::new();
        let mut current = Some(failure);
        while let Some(throwable) = current {
            out.push_str(&format!("T {}\n", throwable.description.replace('\n', "\\n")));
            for frame in &throwable.stack_trace {
                out.push_str(&format!("F {frame}\n"));
                if frame.contains(stop) {
                    break;
                }
            }
            current = throwable.cause.as_deref();
        }
        out
    }

    fn check_stream_fixture(fixture: &str, kind: StreamKind) {
        let mut samples = HashMap::new();
        let mut frames = HashMap::new();
        let mut traces: HashMap<String, String> = HashMap::new();
        let mut cases = Vec::new();
        let mut lines = fixture.lines();
        while let Some(line) = lines.next() {
            let mut parts = line.split(' ');
            match parts.next().unwrap() {
                "sample" => {
                    let name = parts.next().unwrap().to_string();
                    samples.insert(name, unhex(parts.next().unwrap()));
                }
                "frame" => {
                    let id = parts.next().unwrap().to_string();
                    frames.insert(id, line.splitn(3, ' ').nth(2).unwrap().to_string());
                }
                "trace" => {
                    let id = parts.next().unwrap().to_string();
                    let mut text = String::new();
                    for body in lines.by_ref() {
                        if body == "." {
                            break;
                        }
                        match body.strip_prefix("F ") {
                            Some(frame_id) => text.push_str(&format!("F {}\n", frames[frame_id])),
                            None => text.push_str(&format!("{body}\n")),
                        }
                    }
                    traces.insert(format!("#{id}"), text);
                }
                "case" => cases.push(line.to_string()),
                _ => {}
            }
        }
        let render_stream = |failure: &JavaThrowable| {
            let mut out = String::new();
            let mut current = Some(failure);
            while let Some(throwable) = current {
                out.push_str(&format!("T {}\n", throwable.description.replace('\n', "\\n")));
                for frame in &throwable.stack_trace {
                    out.push_str(&format!("F {frame}\n"));
                    if frame.contains("NbtIo.read(NbtIo.java:105)") || frame.contains("RegionFileVersion.wrap(") {
                        break;
                    }
                }
                current = throwable.cause.as_deref();
            }
            out
        };
        let mut mismatches = Vec::new();
        for case in &cases {
            let parts: Vec<&str> = case.split(' ').collect();
            let mut data = samples[parts[1]].clone();
            let label = match parts[2] {
                "full" => "full".to_string(),
                "trunc" => {
                    data.truncate(parts[3].parse().unwrap());
                    format!("trunc {}", parts[3])
                }
                _ => {
                    data[parts[3].parse::<usize>().unwrap()] ^= parts[4].parse::<u8>().unwrap();
                    format!("flip {} {}", parts[3], parts[4])
                }
            };
            let expected = traces.get(parts[parts.len() - 1]).cloned().unwrap_or_else(|| "OK".to_string());
            let actual = match simulate(&data, kind, Mode::Read) {
                Ok(_) => "OK".to_string(),
                Err((failure, _)) => render_stream(&failure),
            };
            if actual != expected {
                let first = expected.lines().zip(actual.lines()).position(|(e, a)| e != a).unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
                mismatches.push(format!("{} {label}: expected {:?} but was {:?}", parts[1], expected.lines().nth(first), actual.lines().nth(first)));
            }
        }
        mismatches.sort();
        mismatches.dedup();
        assert!(mismatches.is_empty(), "{} mismatches of {} cases\n{}", mismatches.len(), cases.len(), mismatches.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn huge_arrays_and_lists_fail_with_out_of_memory_errors() {
        let failure = |bytes: &[u8]| simulate(bytes, StreamKind::None, Mode::Read).map(|_| ()).unwrap_err().0;
        let byte_array = failure(&[10, 0, 0, 7, 0, 1, b'a', 0x7f, 0xff, 0xff, 0xff]);
        assert_eq!(byte_array.description, "java.lang.OutOfMemoryError: Requested array size exceeds VM limit");
        assert_eq!(byte_array.stack_trace[0], "net.minecraft.nbt.ByteArrayTag$1.readAccounted(ByteArrayTag.java:35)");
        let long_array = failure(&[10, 0, 0, 12, 0, 1, b'a', 0x7f, 0xff, 0xff, 0xfe]);
        assert_eq!(long_array.stack_trace[0], "net.minecraft.nbt.LongArrayTag$1.readAccounted(LongArrayTag.java:35)");
        assert_eq!(long_array.description, "java.lang.OutOfMemoryError: Requested array size exceeds VM limit");
        let list = failure(&[10, 0, 0, 9, 0, 1, b'a', 1, 0x7f, 0xff, 0xff, 0xff]);
        assert_eq!(list.stack_trace[..2], ["java.base/java.util.ArrayList.<init>(ArrayList.java:157)", "net.minecraft.nbt.ListTag$1.loadList(ListTag.java:50)"]);
        let truncated = failure(&[10, 0, 0, 7, 0, 1, b'a', 0, 0, 0, 5, 1]);
        assert_eq!(truncated.cause.as_ref().unwrap().description, "java.io.EOFException");
    }

    #[test]
    fn matches_the_java_gzip_stream() {
        check_stream_fixture(include_str!("chunk_stream_gzip_cases.txt"), StreamKind::Gzip);
    }

    #[test]
    fn matches_the_lz4_block_stream() {
        check_stream_fixture(include_str!("chunk_stream_lz4_cases.txt"), StreamKind::Lz4);
    }

    #[test]
    fn matches_the_java_streams_for_truncated_and_corrupted_chunks() {
        let mut samples = HashMap::new();
        let mut frames = HashMap::new();
        let mut traces: HashMap<String, String> = HashMap::new();
        let mut lines = CASES.lines().peekable();
        let mut cases = Vec::new();
        while let Some(line) = lines.next() {
            let mut parts = line.split(' ');
            match parts.next().unwrap() {
                "sample" => {
                    let name = parts.next().unwrap().to_string();
                    samples.insert(name, unhex(parts.next().unwrap()));
                }
                "frame" => {
                    let id = parts.next().unwrap().to_string();
                    frames.insert(id, line.splitn(3, ' ').nth(2).unwrap().to_string());
                }
                "trace" => {
                    let id = parts.next().unwrap().to_string();
                    let mut text = String::new();
                    for body in lines.by_ref() {
                        if body == "." {
                            break;
                        }
                        match body.strip_prefix("F ") {
                            Some(frame_id) => text.push_str(&format!("F {}\n", frames[frame_id])),
                            None => text.push_str(&format!("{body}\n")),
                        }
                    }
                    traces.insert(format!("#{id}"), text);
                }
                "case" | "data" | "scan2" => cases.push(line.to_string()),
                _ => {}
            }
        }
        let expected = |outcome: &str| traces.get(outcome).cloned().unwrap_or_else(|| "OK".to_string());
        let mut mismatches = Vec::new();
        for case in &cases {
            let parts: Vec<&str> = case.split(' ').collect();
            if parts[0] == "scan2" {
                let data = match parts[1] {
                    "trunc" => samples["structures"][..parts[2].parse().unwrap()].to_vec(),
                    _ => {
                        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                        std::io::Write::write_all(&mut encoder, &samples["structuresraw"][..parts[2].parse().unwrap()]).unwrap();
                        encoder.finish().unwrap()
                    }
                };
                for (kind, mode, outcome) in [("old", Mode::Scan(ScanKind::OldData), parts[3]), ("structures", Mode::Scan(ScanKind::Structures), parts[4])] {
                    let actual = match simulate(&data, StreamKind::Deflate, mode) {
                        Ok(_) => "OK".to_string(),
                        Err((failure, _)) => render(&failure, "NbtIo.parse("),
                    };
                    let expected = expected(outcome);
                    if actual != expected {
                        let first = expected.lines().zip(actual.lines()).position(|(e, a)| e != a).unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
                        mismatches.push(format!("scan2 {kind} {} {}: expected {:?} but was {:?}", parts[1], parts[2], expected.lines().nth(first), actual.lines().nth(first)));
                    }
                }
                continue;
            }
            let (label, data, read, scan) = if parts[0] == "data" {
                (parts[1][..16.min(parts[1].len())].to_string(), unhex(parts[1]), parts[2], parts[3])
            } else {
                let mut data = samples[parts[1]].clone();
                match parts[2] {
                    "trunc" => data.truncate(parts[3].parse().unwrap()),
                    _ => {
                        for flip in parts[3].split(',') {
                            let (position, bit) = flip.split_once('.').unwrap();
                            data[position.parse::<usize>().unwrap()] ^= 1 << bit.parse::<u8>().unwrap();
                        }
                    }
                }
                (format!("{} {} {}", parts[1], parts[2], parts[3]), data, parts[parts.len() - 2], parts[parts.len() - 1])
            };
            let actual_read = match simulate(&data, StreamKind::Deflate, Mode::Read) {
                Ok(_) => "OK".to_string(),
                Err((failure, _)) => render(&failure, "NbtIo.read(NbtIo.java:105)"),
            };
            let actual_scan = match simulate(&data, StreamKind::Deflate, Mode::Scan(ScanKind::OldData)) {
                Ok(_) => "OK".to_string(),
                Err((failure, _)) => render(&failure, "NbtIo.parse("),
            };
            for (kind, expected, actual) in [("read", expected(read), actual_read), ("scan", expected(scan), actual_scan)] {
                if actual != expected {
                    let first = expected.lines().zip(actual.lines()).position(|(e, a)| e != a).unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
                    mismatches.push(format!("{kind} {label}: expected {:?} but was {:?}", expected.lines().nth(first), actual.lines().nth(first)));
                }
            }
        }
        mismatches.sort();
        mismatches.dedup();
        assert!(mismatches.is_empty(), "{} mismatches of {} cases\n{}", mismatches.len(), cases.len() * 2, mismatches.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
    }
}

