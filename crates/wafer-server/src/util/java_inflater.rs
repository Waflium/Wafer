const MAX_BITS: usize = 15;
const INPUT_BUFFER_SIZE: usize = 512;

const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DISTANCE_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DISTANCE_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const CODE_LENGTH_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodeType {
    Codes,
    Lens,
    Dists,
}

struct Huffman {
    counts: [u16; MAX_BITS + 1],
    symbols: Vec<u16>,
    max: usize,
}

impl Huffman {
    fn build(lengths: &[u8], code_type: CodeType) -> Option<Huffman> {
        let mut counts = [0u16; MAX_BITS + 1];
        for &length in lengths {
            counts[length as usize] += 1;
        }
        let mut max = MAX_BITS;
        while max >= 1 && counts[max] == 0 {
            max -= 1;
        }
        if max == 0 {
            return Some(Huffman { counts, symbols: Vec::new(), max: 0 });
        }
        let mut left: i32 = 1;
        for length in 1..=MAX_BITS {
            left <<= 1;
            left -= counts[length] as i32;
            if left < 0 {
                return None;
            }
        }
        if left > 0 && (code_type == CodeType::Codes || max != 1) {
            return None;
        }
        let mut offsets = [0u16; MAX_BITS + 2];
        for length in 1..=MAX_BITS {
            offsets[length + 1] = offsets[length] + counts[length];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[offsets[length as usize] as usize] = symbol as u16;
                offsets[length as usize] += 1;
            }
        }
        Some(Huffman { counts, symbols, max })
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    bit: u64,
}

impl BitReader<'_> {
    fn available(&self) -> u64 {
        self.data.len() as u64 * 8 - self.bit
    }

    fn bits(&mut self, count: u32) -> Option<u32> {
        if self.available() < count as u64 {
            return None;
        }
        let mut value = 0u32;
        for i in 0..count {
            let bit = self.bit + i as u64;
            value |= (((self.data[(bit >> 3) as usize] >> (bit & 7)) & 1) as u32) << i;
        }
        self.bit += count as u64;
        Some(value)
    }

    fn align(&mut self) {
        self.bit = (self.bit + 7) & !7;
    }

    fn decode(&mut self, huffman: &Huffman) -> Result<Option<u16>, ()> {
        if huffman.max == 0 {
            return match self.bits(1) {
                Some(_) => Err(()),
                None => Ok(None),
            };
        }
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        let start = self.bit;
        for length in 1..=huffman.max {
            let Some(bit) = self.bits(1) else {
                self.bit = start;
                return Ok(None);
            };
            code |= bit as i32;
            let count = huffman.counts[length] as i32;
            if code - count < first {
                return Ok(Some(huffman.symbols[(index + (code - first)) as usize]));
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unit {
    pub end_bit: u64,
    pub out_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Terminal {
    Truncated,
    Error { bit: u64, message: &'static str },
    Finished { bit: u64, check_ok: bool },
    NeedDictionary { bit: u64 },
}

pub struct Decoded {
    pub output: Vec<u8>,
    pub units: Vec<Unit>,
    pub terminal: Terminal,
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn fixed_tables() -> (Huffman, Huffman) {
    let mut lengths = [0u8; 288];
    lengths[..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..].fill(8);
    let distances = [5u8; 32];
    (Huffman::build(&lengths, CodeType::Lens).expect("fixed literal table"), Huffman::build(&distances, CodeType::Dists).expect("fixed distance table"))
}

pub fn decode_zlib(data: &[u8]) -> Decoded {
    let mut state = Decoder { reader: BitReader { data, bit: 0 }, output: Vec::new(), units: Vec::new() };
    let terminal = state.run();
    Decoded { output: state.output, units: state.units, terminal }
}

struct Decoder<'a> {
    reader: BitReader<'a>,
    output: Vec<u8>,
    units: Vec<Unit>,
}

impl Decoder<'_> {
    fn error(&self, message: &'static str) -> Terminal {
        Terminal::Error { bit: self.reader.bit, message }
    }

    fn run(&mut self) -> Terminal {
        let Some(header) = self.reader.bits(16) else { return Terminal::Truncated };
        let (cmf, flg) = (header & 0xff, header >> 8);
        if ((cmf << 8) + flg) % 31 != 0 {
            return self.error("incorrect header check");
        }
        if cmf & 0xf != 8 {
            return self.error("unknown compression method");
        }
        if (cmf >> 4) + 8 > 15 {
            return self.error("invalid window size");
        }
        if flg & 0x20 != 0 {
            return match self.reader.bits(32) {
                Some(_) => Terminal::NeedDictionary { bit: self.reader.bit },
                None => Terminal::Truncated,
            };
        }
        loop {
            let Some(last) = self.reader.bits(1) else { return Terminal::Truncated };
            let Some(block_type) = self.reader.bits(2) else { return Terminal::Truncated };
            let result = match block_type {
                0 => self.stored(),
                1 => {
                    let (lengths, distances) = fixed_tables();
                    self.codes(&lengths, &distances)
                }
                2 => self.dynamic(),
                _ => Err(self.error("invalid block type")),
            };
            if let Err(terminal) = result {
                return terminal;
            }
            if last == 1 {
                break;
            }
        }
        self.reader.align();
        let Some(check) = self.reader.bits(32) else { return Terminal::Truncated };
        let expected = adler32(&self.output).swap_bytes();
        Terminal::Finished { bit: self.reader.bit, check_ok: check == expected }
    }

    fn stored(&mut self) -> Result<(), Terminal> {
        self.reader.align();
        let Some(lengths) = self.reader.bits(32) else { return Err(Terminal::Truncated) };
        if lengths & 0xffff != (lengths >> 16) ^ 0xffff {
            return Err(self.error("invalid stored block lengths"));
        }
        for _ in 0..(lengths & 0xffff) {
            let Some(byte) = self.reader.bits(8) else { return Err(Terminal::Truncated) };
            self.output.push(byte as u8);
            self.units.push(Unit { end_bit: self.reader.bit, out_end: self.output.len() });
        }
        Ok(())
    }

    fn dynamic(&mut self) -> Result<(), Terminal> {
        let Some(counts) = self.reader.bits(14) else { return Err(Terminal::Truncated) };
        let (nlen, ndist, ncode) = ((counts & 0x1f) as usize + 257, ((counts >> 5) & 0x1f) as usize + 1, ((counts >> 10) & 0xf) as usize + 4);
        if nlen > 286 || ndist > 30 {
            return Err(self.error("too many length or distance symbols"));
        }
        let mut code_lengths = [0u8; 19];
        for &index in &CODE_LENGTH_ORDER[..ncode] {
            let Some(length) = self.reader.bits(3) else { return Err(Terminal::Truncated) };
            code_lengths[index] = length as u8;
        }
        let Some(code_table) = Huffman::build(&code_lengths, CodeType::Codes) else { return Err(self.error("invalid code lengths set")) };
        let mut lengths = vec![0u8; nlen + ndist];
        let mut have = 0;
        while have < nlen + ndist {
            let symbol = match self.reader.decode(&code_table) {
                Ok(Some(symbol)) => symbol,
                Ok(None) => return Err(Terminal::Truncated),
                Err(()) => 0,
            };
            if symbol < 16 {
                lengths[have] = symbol as u8;
                have += 1;
                continue;
            }
            let (extra_bits, base, previous) = match symbol {
                16 => (2, 3, have.checked_sub(1).map(|index| lengths[index])),
                17 => (3, 3, Some(0)),
                _ => (7, 11, Some(0)),
            };
            let Some(extra) = self.reader.bits(extra_bits) else { return Err(Terminal::Truncated) };
            let Some(value) = previous else { return Err(self.error("invalid bit length repeat")) };
            let copy = (extra + base) as usize;
            if have + copy > nlen + ndist {
                return Err(self.error("invalid bit length repeat"));
            }
            lengths[have..have + copy].fill(value);
            have += copy;
        }
        if lengths[256] == 0 {
            return Err(self.error("invalid code -- missing end-of-block"));
        }
        let Some(length_table) = Huffman::build(&lengths[..nlen], CodeType::Lens) else { return Err(self.error("invalid literal/lengths set")) };
        let Some(distance_table) = Huffman::build(&lengths[nlen..], CodeType::Dists) else { return Err(self.error("invalid distances set")) };
        self.codes(&length_table, &distance_table)
    }

    fn codes(&mut self, length_table: &Huffman, distance_table: &Huffman) -> Result<(), Terminal> {
        loop {
            let symbol = match self.reader.decode(length_table) {
                Ok(Some(symbol)) => symbol,
                Ok(None) => return Err(Terminal::Truncated),
                Err(()) => return Err(self.error("invalid literal/length code")),
            };
            if symbol < 256 {
                self.output.push(symbol as u8);
                self.units.push(Unit { end_bit: self.reader.bit, out_end: self.output.len() });
                continue;
            }
            if symbol == 256 {
                return Ok(());
            }
            let index = symbol as usize - 257;
            if index >= LENGTH_BASE.len() {
                return Err(self.error("invalid literal/length code"));
            }
            let Some(length_extra) = self.reader.bits(LENGTH_EXTRA[index] as u32) else { return Err(Terminal::Truncated) };
            let length = LENGTH_BASE[index] as usize + length_extra as usize;
            let distance_symbol = match self.reader.decode(distance_table) {
                Ok(Some(symbol)) => symbol as usize,
                Ok(None) => return Err(Terminal::Truncated),
                Err(()) => return Err(self.error("invalid distance code")),
            };
            if distance_symbol >= DISTANCE_BASE.len() {
                return Err(self.error("invalid distance code"));
            }
            let Some(distance_extra) = self.reader.bits(DISTANCE_EXTRA[distance_symbol] as u32) else { return Err(Terminal::Truncated) };
            let distance = DISTANCE_BASE[distance_symbol] as usize + distance_extra as usize;
            if distance > self.output.len() {
                return Err(self.error("invalid distance too far back"));
            }
            for _ in 0..length {
                let byte = self.output[self.output.len() - distance];
                self.output.push(byte);
            }
            self.units.push(Unit { end_bit: self.reader.bit, out_end: self.output.len() });
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InflaterFailure {
    ZipException(String),
    Eof,
}

pub struct InflaterInputStream {
    decoded: Decoded,
    stream_length: usize,
    fed: usize,
    delivered: usize,
    finished: bool,
    needs_dictionary: bool,
    reached_eof: bool,
}

impl InflaterInputStream {
    pub fn new(compressed: &[u8]) -> InflaterInputStream {
        InflaterInputStream { decoded: decode_zlib(compressed), stream_length: compressed.len(), fed: 0, delivered: 0, finished: false, needs_dictionary: false, reached_eof: false }
    }

    pub fn output(&self) -> &[u8] {
        &self.decoded.output
    }

    fn decodable_end(&self) -> (usize, bool) {
        let available_bits = self.fed as u64 * 8;
        let ready = self.decoded.units.partition_point(|unit| unit.end_bit <= available_bits);
        let out_end = if ready == 0 { 0 } else { self.decoded.units[ready - 1].out_end };
        (out_end, ready == self.decoded.units.len())
    }

    fn inflate(&mut self, length: usize) -> Result<usize, InflaterFailure> {
        let (decodable, all_units) = self.decodable_end();
        let pending = decodable - self.delivered;
        let produced = pending.min(length);
        if produced < pending || !all_units {
            self.delivered += produced;
            return Ok(produced);
        }
        let available_bits = self.fed as u64 * 8;
        match &self.decoded.terminal {
            Terminal::Error { bit, message } if *bit <= available_bits => Err(InflaterFailure::ZipException((*message).to_string())),
            Terminal::Finished { bit, check_ok } if *bit <= available_bits => {
                if *check_ok {
                    self.finished = true;
                    self.delivered += produced;
                    Ok(produced)
                } else {
                    Err(InflaterFailure::ZipException("incorrect data check".to_string()))
                }
            }
            Terminal::NeedDictionary { bit } if *bit <= available_bits => {
                self.needs_dictionary = true;
                self.delivered += produced;
                Ok(produced)
            }
            _ => {
                self.delivered += produced;
                Ok(produced)
            }
        }
    }

    pub fn read(&mut self, length: usize) -> Result<Option<(usize, usize)>, InflaterFailure> {
        loop {
            let start = self.delivered;
            let produced = self.inflate(length)?;
            if produced > 0 {
                return Ok(Some((start, start + produced)));
            }
            if self.finished || self.needs_dictionary {
                self.reached_eof = true;
                return Ok(None);
            }
            if self.fed >= self.stream_length {
                return Err(InflaterFailure::Eof);
            }
            self.fed = (self.fed + INPUT_BUFFER_SIZE).min(self.stream_length);
        }
    }

    pub fn skip(&mut self, count: usize) -> Result<usize, InflaterFailure> {
        let mut total = 0;
        while total < count {
            let length = (count - total).min(INPUT_BUFFER_SIZE);
            match self.read(length)? {
                Some((start, end)) => total += end - start,
                None => {
                    self.reached_eof = true;
                    break;
                }
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure_of(data: &[u8]) -> String {
        let mut stream = InflaterInputStream::new(data);
        match stream.read(8192) {
            Err(InflaterFailure::ZipException(message)) => message,
            Err(InflaterFailure::Eof) => "EOF".to_string(),
            Ok(_) => "OK".to_string(),
        }
    }

    #[test]
    fn maps_zlib_failures_to_java_messages() {
        assert_eq!(failure_of(&[0xab, 0xab, 0xab]), "incorrect header check");
        assert_eq!(failure_of(&[0x79, 0x18, 0x00]), "unknown compression method");
        assert_eq!(failure_of(&[0x88, 0x1c, 0x00]), "invalid window size");
        assert_eq!(failure_of(&[0x78, 0x01, 0x07]), "invalid block type");
        assert_eq!(failure_of(&[0x78, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00]), "invalid stored block lengths");
        assert_eq!(failure_of(&[0x78, 0x01, 0xfd, 0x1f, 0x00]), "too many length or distance symbols");
        assert_eq!(failure_of(&[0x78, 0x01]), "EOF");
        assert_eq!(failure_of(&[]), "EOF");
    }

    #[test]
    fn reports_a_bad_checksum_and_drops_the_output_of_that_call() {
        let stored = [0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x41, 0x42, 0x00, 0x00, 0x00, 0x00];
        let mut stream = InflaterInputStream::new(&stored);
        assert_eq!(stream.read(8192), Err(InflaterFailure::ZipException("incorrect data check".to_string())));
        let mut good = stored;
        good[9..].copy_from_slice(&adler32(b"AB").to_be_bytes());
        let mut stream = InflaterInputStream::new(&good);
        assert_eq!(stream.read(8192), Ok(Some((0, 2))));
        assert_eq!(stream.read(8192), Ok(None));
        assert_eq!(stream.output(), b"AB");
    }

    #[test]
    fn feeds_the_inflater_in_windows_of_512_bytes() {
        let mut stored = vec![0x78, 0x01, 0x01, 0x00, 0x04, 0xff, 0xfb];
        stored.extend((0..1024u32).map(|i| (i % 251) as u8));
        let expected: Vec<u8> = (0..1024u32).map(|i| (i % 251) as u8).collect();
        stored.extend(adler32(&expected).to_be_bytes());
        let mut stream = InflaterInputStream::new(&stored);
        let mut total = 0;
        while let Some((start, end)) = stream.read(8192).unwrap() {
            assert!(end - start <= 512);
            total += end - start;
        }
        assert_eq!(total, 1024);
    }
}
