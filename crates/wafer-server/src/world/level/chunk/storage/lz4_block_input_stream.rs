use super::region_file_version::lz4_block;
use crate::util::java_throwable::JavaThrowable;

const MAGIC: &[u8; 8] = b"LZ4Block";
const MAGIC_LENGTH: usize = 8;
const HEADER_LENGTH: usize = MAGIC_LENGTH + 13;
const COMPRESSION_LEVEL_BASE: u32 = 10;
const COMPRESSION_METHOD_RAW: u8 = 0x10;
const COMPRESSION_METHOD_LZ4: u8 = 0x20;
const COPY_LENGTH: i32 = 8;
const RUN_MASK: i32 = 15;
const ML_BITS: u32 = 4;
const ML_MASK: i32 = 15;
const MIN_MATCH: i32 = 4;

fn block_frame(line: u32) -> &'static str {
    match line {
        241 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:241)",
        247 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:247)",
        254 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:254)",
        266 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:266)",
        270 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:270)",
        284 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:284)",
        290 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:290)",
        298 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:298)",
        300 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:300)",
        304 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:304)",
        313 => "net.jpountz.lz4.LZ4BlockInputStream.refill(LZ4BlockInputStream.java:313)",
        _ => unreachable!(),
    }
}

const READ_FULLY_EOF: &str = "net.jpountz.lz4.LZ4BlockInputStream.readFully(LZ4BlockInputStream.java:335)";
const READ_REFILL: &str = "net.jpountz.lz4.LZ4BlockInputStream.read(LZ4BlockInputStream.java:204)";
const SKIP_REFILL: &str = "net.jpountz.lz4.LZ4BlockInputStream.skip(LZ4BlockInputStream.java:226)";
const DECOMPRESS_FAST: &str = "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:26)";

fn decompressor_frame(line: u32) -> &'static str {
    match line {
        50 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:50)",
        62 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:62)",
        71 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:71)",
        77 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:77)",
        79 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:79)",
        99 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:99)",
        108 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:108)",
        118 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:118)",
        123 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:123)",
        130 => "net.jpountz.lz4.LZ4JavaSafeFastDecompressor.decompress(LZ4JavaSafeFastDecompressor.java:130)",
        _ => unreachable!(),
    }
}

struct Lz4Exception {
    message: String,
    line: u32,
}

fn not_enough_space(available: i32, required: i32) -> bool {
    available < required
}

fn decompress_fast(src: &[u8], dest: &mut [u8], dest_len: i32) -> Result<i32, Lz4Exception> {
    let src_end = src.len() as i32;
    let dest_end = dest_len;
    let mut s_off: i32 = 0;
    let mut d_off: i32 = 0;
    let malformed = |s_off: i32, line: u32| Lz4Exception { message: format!("Malformed input at {s_off}"), line };
    loop {
        if s_off >= src_end {
            return Err(malformed(s_off, 50));
        }
        let token = src[s_off as usize] as i32;
        s_off += 1;
        let mut literal_len = token >> ML_BITS;
        if literal_len == RUN_MASK {
            let mut len: i8 = -1;
            while s_off < src_end {
                len = src[s_off as usize] as i8;
                s_off += 1;
                if len != -1 {
                    break;
                }
                literal_len = literal_len.wrapping_add(0xFF);
                if literal_len < 0 {
                    return Err(Lz4Exception { message: "Too large literalLen".to_string(), line: 62 });
                }
            }
            literal_len = literal_len.wrapping_add(len as u8 as i32);
        }
        let literal_copy_end = d_off.wrapping_add(literal_len);
        if literal_copy_end < d_off {
            return Err(Lz4Exception { message: "Too large literalLen".to_string(), line: 71 });
        }
        if not_enough_space(dest_end.wrapping_sub(literal_copy_end), COPY_LENGTH) || not_enough_space(src_end.wrapping_sub(s_off), COPY_LENGTH.wrapping_add(literal_len)) {
            if literal_copy_end != dest_end {
                return Err(malformed(s_off, 77));
            } else if not_enough_space(src_end - s_off, literal_len) {
                return Err(malformed(s_off, 79));
            }
            dest[d_off as usize..literal_copy_end as usize].copy_from_slice(&src[s_off as usize..(s_off + literal_len) as usize]);
            s_off += literal_len;
            break;
        }
        dest[d_off as usize..literal_copy_end as usize].copy_from_slice(&src[s_off as usize..(s_off + literal_len) as usize]);
        s_off += literal_len;
        d_off = literal_copy_end;
        let match_dec = src[s_off as usize] as i32 | (src[s_off as usize + 1] as i32) << 8;
        s_off += 2;
        let match_off = d_off - match_dec;
        if match_off < 0 {
            return Err(malformed(s_off, 99));
        }
        let mut match_len = token & ML_MASK;
        if match_len == ML_MASK {
            let mut len: i8 = -1;
            while s_off < src_end {
                len = src[s_off as usize] as i8;
                s_off += 1;
                if len != -1 {
                    break;
                }
                match_len = match_len.wrapping_add(0xFF);
                if match_len < 0 {
                    return Err(Lz4Exception { message: "Too large matchLen".to_string(), line: 108 });
                }
            }
            match_len = match_len.wrapping_add(len as u8 as i32);
        }
        match_len = match_len.wrapping_add(MIN_MATCH);
        let match_copy_end = d_off.wrapping_add(match_len);
        if match_copy_end < d_off {
            return Err(Lz4Exception { message: "Too large matchLen".to_string(), line: 118 });
        }
        if match_dec == 0 {
            if match_copy_end > dest_end {
                return Err(malformed(s_off, 123));
            }
            dest[d_off as usize..match_copy_end as usize].fill(0);
        } else {
            if not_enough_space(dest_end - match_copy_end, COPY_LENGTH) && match_copy_end > dest_end {
                return Err(malformed(s_off, 130));
            }
            for i in 0..match_len {
                dest[(d_off + i) as usize] = dest[(match_off + i) as usize];
            }
        }
        d_off = match_copy_end;
    }
    Ok(s_off)
}

fn io_exception(line: u32) -> JavaThrowable {
    JavaThrowable::new("java.io.IOException: Stream is corrupted", vec![block_frame(line)])
}

pub struct Lz4BlockInputStream {
    data: Vec<u8>,
    position: usize,
    compressed_buffer: Vec<u8>,
    buffer: Vec<u8>,
    output: Vec<u8>,
    block_start: usize,
    original_len: usize,
    o: usize,
    finished: bool,
}

impl Lz4BlockInputStream {
    pub fn new(data: &[u8]) -> Lz4BlockInputStream {
        Lz4BlockInputStream {
            data: data.to_vec(),
            position: 0,
            compressed_buffer: vec![0; HEADER_LENGTH],
            buffer: Vec::new(),
            output: Vec::new(),
            block_start: 0,
            original_len: 0,
            o: 0,
            finished: false,
        }
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    fn try_read_fully(&mut self, length: usize, into_compressed: bool) -> bool {
        if self.data.len() - self.position < length {
            self.position = self.data.len();
            return false;
        }
        if into_compressed {
            self.compressed_buffer[..length].copy_from_slice(&self.data[self.position..self.position + length]);
        }
        self.position += length;
        true
    }

    fn refill(&mut self) -> Result<(), JavaThrowable> {
        if !self.try_read_fully(HEADER_LENGTH, true) {
            return Err(JavaThrowable::new("java.io.EOFException: Stream ended prematurely", vec![block_frame(241)]));
        }
        if &self.compressed_buffer[..MAGIC_LENGTH] != MAGIC {
            return Err(io_exception(247));
        }
        let token = self.compressed_buffer[MAGIC_LENGTH];
        let compression_method = token & 0xF0;
        let compression_level = COMPRESSION_LEVEL_BASE + (token & 0x0F) as u32;
        if compression_method != COMPRESSION_METHOD_RAW && compression_method != COMPRESSION_METHOD_LZ4 {
            return Err(io_exception(254));
        }
        let int = |buffer: &[u8], at: usize| i32::from_le_bytes(buffer[at..at + 4].try_into().expect("four bytes"));
        let compressed_len = int(&self.compressed_buffer, MAGIC_LENGTH + 1);
        let original_len = int(&self.compressed_buffer, MAGIC_LENGTH + 5);
        let check = int(&self.compressed_buffer, MAGIC_LENGTH + 9);
        self.original_len = original_len.max(0) as usize;
        if original_len as i64 > 1i64 << compression_level
            || original_len < 0
            || compressed_len < 0
            || (original_len == 0 && compressed_len != 0)
            || (original_len != 0 && compressed_len == 0)
            || (compression_method == COMPRESSION_METHOD_RAW && original_len != compressed_len)
        {
            return Err(io_exception(266));
        }
        if original_len == 0 && compressed_len == 0 {
            if check != 0 {
                return Err(io_exception(270));
            }
            self.finished = true;
            return Ok(());
        }
        let (original_len, compressed_len) = (original_len as usize, compressed_len as usize);
        if self.buffer.len() < original_len {
            self.buffer = vec![0; original_len.max(self.buffer.len() * 3 / 2)];
        }
        match compression_method {
            COMPRESSION_METHOD_RAW => {
                if self.data.len() - self.position < original_len {
                    self.position = self.data.len();
                    return Err(JavaThrowable::new("java.io.EOFException: Stream ended prematurely", vec![READ_FULLY_EOF, block_frame(284)]));
                }
                self.buffer[..original_len].copy_from_slice(&self.data[self.position..self.position + original_len]);
                self.position += original_len;
            }
            _ => {
                if self.compressed_buffer.len() < compressed_len {
                    self.compressed_buffer.resize(compressed_len.max(self.compressed_buffer.len() * 3 / 2), 0);
                }
                if !self.try_read_fully(compressed_len, true) {
                    return Err(JavaThrowable::new("java.io.EOFException: Stream ended prematurely", vec![READ_FULLY_EOF, block_frame(290)]));
                }
                match decompress_fast(&self.compressed_buffer, &mut self.buffer, original_len as i32) {
                    Err(exception) => {
                        let cause = JavaThrowable::new(format!("net.jpountz.lz4.LZ4Exception: {}", exception.message), vec![decompressor_frame(exception.line), DECOMPRESS_FAST, block_frame(298)]);
                        return Err(JavaThrowable::new("java.io.IOException: Stream is corrupted", vec![block_frame(304)]).with_cause(cause));
                    }
                    Ok(consumed) if consumed as usize != compressed_len => return Err(io_exception(300)),
                    Ok(_) => {}
                }
            }
        }
        if (lz4_block::checksum(&self.buffer[..original_len]) as i32) != check {
            return Err(io_exception(313));
        }
        self.block_start = self.output.len();
        self.output.extend_from_slice(&self.buffer[..original_len]);
        self.o = 0;
        Ok(())
    }

    pub fn read(&mut self, length: usize) -> Result<Option<(usize, usize)>, JavaThrowable> {
        if self.finished {
            return Ok(None);
        }
        if self.o == self.original_len {
            self.refill().map_err(|mut failure| {
                failure.push_frames(&[READ_REFILL]);
                failure
            })?;
        }
        if self.finished {
            return Ok(None);
        }
        let length = length.min(self.original_len - self.o);
        let start = self.block_start + self.o;
        self.o += length;
        Ok(Some((start, start + length)))
    }

    pub fn skip(&mut self, count: usize) -> Result<usize, JavaThrowable> {
        if count == 0 || self.finished {
            return Ok(0);
        }
        if self.o == self.original_len {
            self.refill().map_err(|mut failure| {
                failure.push_frames(&[SKIP_REFILL]);
                failure
            })?;
        }
        if self.finished {
            return Ok(0);
        }
        let skipped = count.min(self.original_len - self.o);
        self.o += skipped;
        Ok(skipped)
    }
}
