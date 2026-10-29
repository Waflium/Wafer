use flate2::Crc;

use super::java_inflater::{InflaterFailure, InflaterInputStream};
use super::java_throwable::JavaThrowable;

const READ_UBYTE: &str = "java.base/java.util.zip.GZIPInputStream.readUByte(GZIPInputStream.java:332)";
const READ_USHORT_LOW: &str = "java.base/java.util.zip.GZIPInputStream.readUShort(GZIPInputStream.java:322)";
const READ_USHORT_HIGH: &str = "java.base/java.util.zip.GZIPInputStream.readUShort(GZIPInputStream.java:323)";
const SKIP_BYTES: &str = "java.base/java.util.zip.GZIPInputStream.skipBytes(GZIPInputStream.java:356)";
const READ_UINT_LOW: &str = "java.base/java.util.zip.GZIPInputStream.readUInt(GZIPInputStream.java:314)";
const READ_UINT_HIGH: &str = "java.base/java.util.zip.GZIPInputStream.readUInt(GZIPInputStream.java:315)";
const HEADER_MAGIC: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:227)";
const HEADER_NOT_GZIP: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:231)";
const HEADER_METHOD: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:234)";
const HEADER_UNSUPPORTED: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:235)";
const HEADER_FLAGS: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:238)";
const HEADER_SKIP_FIXED: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:240)";
const HEADER_EXTRA_LENGTH: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:244)";
const HEADER_EXTRA_SKIP: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:245)";
const HEADER_NAME: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:252)";
const HEADER_COMMENT: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:258)";
const HEADER_CRC: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:263)";
const HEADER_CORRUPT: &str = "java.base/java.util.zip.GZIPInputStream.readHeader(GZIPInputStream.java:264)";
const INIT_SIZED: &str = "java.base/java.util.zip.GZIPInputStream.<init>(GZIPInputStream.java:86)";
const INIT_DEFAULT: &str = "java.base/java.util.zip.GZIPInputStream.<init>(GZIPInputStream.java:118)";
const READ_INFLATE: &str = "java.base/java.util.zip.GZIPInputStream.read(GZIPInputStream.java:155)";
const READ_TRAILER_CALL: &str = "java.base/java.util.zip.GZIPInputStream.read(GZIPInputStream.java:157)";
const TRAILER_FIRST: &str = "java.base/java.util.zip.GZIPInputStream.readTrailer(GZIPInputStream.java:288)";
const TRAILER_SECOND: &str = "java.base/java.util.zip.GZIPInputStream.readTrailer(GZIPInputStream.java:290)";
const TRAILER_CORRUPT: &str = "java.base/java.util.zip.GZIPInputStream.readTrailer(GZIPInputStream.java:291)";
const IIS_READ_ZIP: &str = "java.base/java.util.zip.InflaterInputStream.read(InflaterInputStream.java:214)";
const IIS_FILL: &str = "java.base/java.util.zip.InflaterInputStream.fill(InflaterInputStream.java:310)";
const IIS_READ_FILL: &str = "java.base/java.util.zip.InflaterInputStream.read(InflaterInputStream.java:208)";
const IIS_SKIP: &str = "java.base/java.util.zip.InflaterInputStream.skip(InflaterInputStream.java:270)";
const SKIP_BUFFER: usize = 512;
const GZIP_MAGIC: u16 = 0x8b1f;
const FHCRC: u8 = 2;
const FEXTRA: u8 = 4;
const FNAME: u8 = 8;
const FCOMMENT: u8 = 16;

struct HeaderReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl HeaderReader<'_> {
    fn eof(frames: &[&'static str]) -> JavaThrowable {
        JavaThrowable::new("java.io.EOFException", frames.to_vec())
    }

    fn ubyte(&mut self, callers: &[&'static str]) -> Result<u8, JavaThrowable> {
        let Some(&byte) = self.data.get(self.position) else {
            let mut frames = vec![READ_UBYTE];
            frames.extend_from_slice(callers);
            return Err(Self::eof(&frames));
        };
        self.position += 1;
        Ok(byte)
    }

    fn ushort(&mut self, caller: &'static str) -> Result<u16, JavaThrowable> {
        let low = self.ubyte(&[READ_USHORT_LOW, caller])?;
        let high = self.ubyte(&[READ_USHORT_HIGH, caller])?;
        Ok(((high as u16) << 8) | low as u16)
    }

    fn skip(&mut self, count: usize, caller: &'static str) -> Result<(), JavaThrowable> {
        if self.data.len() - self.position < count {
            return Err(Self::eof(&[SKIP_BYTES, caller]));
        }
        self.position += count;
        Ok(())
    }
}

fn parse_header(data: &[u8]) -> Result<usize, JavaThrowable> {
    let mut reader = HeaderReader { data, position: 0 };
    let magic = reader.ushort(HEADER_MAGIC)?;
    if magic != GZIP_MAGIC {
        return Err(JavaThrowable::new("java.util.zip.ZipException: Not in GZIP format", vec![HEADER_NOT_GZIP]));
    }
    if reader.ubyte(&[HEADER_METHOD])? != 8 {
        return Err(JavaThrowable::new("java.util.zip.ZipException: Unsupported compression method", vec![HEADER_UNSUPPORTED]));
    }
    let flags = reader.ubyte(&[HEADER_FLAGS])?;
    reader.skip(6, HEADER_SKIP_FIXED)?;
    if flags & FEXTRA != 0 {
        let length = reader.ushort(HEADER_EXTRA_LENGTH)? as usize;
        reader.skip(length, HEADER_EXTRA_SKIP)?;
    }
    if flags & FNAME != 0 {
        while reader.ubyte(&[HEADER_NAME])? != 0 {}
    }
    if flags & FCOMMENT != 0 {
        while reader.ubyte(&[HEADER_COMMENT])? != 0 {}
    }
    if flags & FHCRC != 0 {
        let mut crc = Crc::new();
        crc.update(&data[..reader.position]);
        let expected = (crc.sum() & 0xffff) as u16;
        if reader.ushort(HEADER_CRC)? != expected {
            return Err(JavaThrowable::new("java.util.zip.ZipException: Corrupt GZIP header", vec![HEADER_CORRUPT]));
        }
    }
    Ok(reader.position)
}

struct Member {
    inflater: InflaterInputStream,
    start: usize,
    base: usize,
    appended: usize,
}

pub struct GzipInputStream {
    data: Vec<u8>,
    member: Member,
    output: Vec<u8>,
    eos: bool,
}

impl GzipInputStream {
    pub fn new(data: &[u8]) -> Result<GzipInputStream, JavaThrowable> {
        let header = parse_header(data).map_err(|failure| failure.with_frames(&[INIT_SIZED, INIT_DEFAULT]))?;
        Ok(GzipInputStream {
            data: data.to_vec(),
            member: Member { inflater: InflaterInputStream::new_raw(&data[header..], 0), start: header, base: 0, appended: 0 },
            output: Vec::new(),
            eos: false,
        })
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    fn inflater_failure(failure: InflaterFailure, skipping: bool) -> JavaThrowable {
        let (description, mut frames): (String, Vec<&'static str>) = match failure {
            InflaterFailure::ZipException(message) => (format!("java.util.zip.ZipException: {message}"), vec![IIS_READ_ZIP]),
            InflaterFailure::Eof => ("java.io.EOFException: Unexpected end of ZLIB input stream".to_string(), vec![IIS_FILL, IIS_READ_FILL]),
        };
        frames.push(READ_INFLATE);
        if skipping {
            frames.push(IIS_SKIP);
        }
        JavaThrowable::new(description, frames)
    }

    pub fn read(&mut self, length: usize, skipping: bool) -> Result<Option<(usize, usize)>, JavaThrowable> {
        loop {
            if self.eos {
                return Ok(None);
            }
            match self.member.inflater.read(length) {
                Ok(Some((start, end))) => {
                    let member = &mut self.member;
                    self.output.extend_from_slice(&member.inflater.output()[member.appended..end]);
                    member.appended = end;
                    return Ok(Some((member.base + start, member.base + end)));
                }
                Ok(None) => {
                    if self.read_trailer().map_err(|failure| if skipping { failure.with_frames(&[IIS_SKIP]) } else { failure })? {
                        self.eos = true;
                        return Ok(None);
                    }
                }
                Err(failure) => return Err(Self::inflater_failure(failure, skipping)),
            }
        }
    }

    pub fn skip(&mut self, count: usize) -> Result<usize, JavaThrowable> {
        let mut total = 0;
        while total < count {
            match self.read((count - total).min(SKIP_BUFFER), true)? {
                Some((start, end)) => total += end - start,
                None => break,
            }
        }
        Ok(total)
    }

    fn read_unsigned_int(&self, position: usize, trailer_site: &'static str) -> Result<u32, JavaThrowable> {
        let available = self.data.len().saturating_sub(position).min(4);
        if available < 4 {
            let (caller, uint) = if available < 2 { (if available == 0 { READ_USHORT_LOW } else { READ_USHORT_HIGH }, READ_UINT_LOW) } else { (if available == 2 { READ_USHORT_LOW } else { READ_USHORT_HIGH }, READ_UINT_HIGH) };
            return Err(JavaThrowable::new("java.io.EOFException", vec![READ_UBYTE, caller, uint, trailer_site, READ_TRAILER_CALL]));
        }
        Ok(u32::from_le_bytes(self.data[position..position + 4].try_into().expect("four bytes")))
    }

    fn read_trailer(&mut self) -> Result<bool, JavaThrowable> {
        let consumed = self.member.inflater.consumed_bytes().expect("finished inflater");
        let trailer = self.member.start + consumed;
        let member_output = self.member.inflater.output();
        let mut crc = Crc::new();
        crc.update(member_output);
        let written = member_output.len() as u32;
        let corrupt = || JavaThrowable::new("java.util.zip.ZipException: Corrupt GZIP trailer", vec![TRAILER_CORRUPT, READ_TRAILER_CALL]);
        if self.read_unsigned_int(trailer, TRAILER_FIRST)? != crc.sum() || self.read_unsigned_int(trailer + 4, TRAILER_SECOND)? != written {
            return Err(corrupt());
        }
        let after = trailer + 8;
        let next_start = match self.data.get(after..).filter(|rest| !rest.is_empty()) {
            None => return Ok(true),
            Some(rest) => match parse_header(rest) {
                Ok(header) => after + header,
                Err(_) => return Ok(true),
            },
        };
        let remaining = (self.member.inflater.fed() + self.member.start).saturating_sub(next_start);
        self.member = Member {
            inflater: InflaterInputStream::new_raw(&self.data[next_start..], remaining),
            start: next_start,
            base: self.output.len(),
            appended: 0,
        };
        Ok(false)
    }
}
