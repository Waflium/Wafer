use wafer_core::java_string::utf16_units;

use super::java_throwable::JavaThrowable;

const DEFAULT_MAX_BUFFER_SIZE: usize = 8192;
const INITIAL_BYTE_BUFFER_CAPACITY: usize = 512;
const MAX_BYTE_BUFFER_CAPACITY: usize = 8192;
const MALFORMED_INPUT: &str = "java.nio.charset.MalformedInputException: Input length = 1";
const THROW_EXCEPTION: &str = "java.base/java.nio.charset.CoderResult.throwException(CoderResult.java:279)";
const IMPL_WRITE_ENCODE: &str = "java.base/sun.nio.cs.StreamEncoder.implWrite(StreamEncoder.java:294)";
const FLUSH_LEFTOVER_CHAR: &str = "java.base/sun.nio.cs.StreamEncoder.flushLeftoverChar(StreamEncoder.java:258)";
const IMPL_WRITE_LEFTOVER: &str = "java.base/sun.nio.cs.StreamEncoder.implWrite(StreamEncoder.java:274)";
const IMPL_WRITE_WRAP: &str = "java.base/sun.nio.cs.StreamEncoder.implWrite(StreamEncoder.java:267)";
const STREAM_ENCODER_WRITE: &str = "java.base/sun.nio.cs.StreamEncoder.write(StreamEncoder.java:122)";
const OUTPUT_STREAM_WRITER_WRITE: &str = "java.base/java.io.OutputStreamWriter.write(OutputStreamWriter.java:206)";
const FLUSH_BUFFER: &str = "java.base/java.io.BufferedWriter.flushBuffer(BufferedWriter.java:168)";
const BUFFERED_WRITER_WRITE: &str = "java.base/java.io.BufferedWriter.write(BufferedWriter.java:282)";
const WRITER_WRITE: &str = "java.base/java.io.Writer.write(Writer.java:248)";
const BUFFERED_WRITER_FLUSH: &str = "java.base/java.io.BufferedWriter.flush(BufferedWriter.java:305)";
const BUFFERED_WRITER_CLOSE_FLUSH: &str = "java.base/java.io.BufferedWriter.close(BufferedWriter.java:317)";
const IMPL_CLOSE: &str = "java.base/sun.nio.cs.StreamEncoder.implClose(StreamEncoder.java:326)";
const STREAM_ENCODER_CLOSE: &str = "java.base/sun.nio.cs.StreamEncoder.close(StreamEncoder.java:159)";
const OUTPUT_STREAM_WRITER_CLOSE: &str = "java.base/java.io.OutputStreamWriter.close(OutputStreamWriter.java:253)";
const BUFFERED_WRITER_CLOSE_AFTER_FAILURE: &str = "java.base/java.io.BufferedWriter.close(BufferedWriter.java:316)";
const BUFFERED_WRITER_CLOSE: &str = "java.base/java.io.BufferedWriter.close(BufferedWriter.java:318)";

#[derive(Clone, Copy)]
enum Malformed {
    Encode,
    Leftover,
}

struct StreamEncoder {
    bb: Vec<u8>,
    bb_capacity: usize,
    have_leftover_char: bool,
    leftover_char: u16,
    written: Vec<u8>,
    closed: bool,
}

impl StreamEncoder {
    fn write_bytes(&mut self) {
        self.written.append(&mut self.bb);
    }

    fn put(&mut self, bytes: &[u8]) {
        if self.bb.len() + bytes.len() > self.bb_capacity {
            self.write_bytes();
        }
        self.bb.extend_from_slice(bytes);
    }

    fn put_code_point(&mut self, code_point: u32) {
        let mut buffer = [0u8; 4];
        let c = char::from_u32(code_point).expect("a scalar value");
        let encoded = c.encode_utf8(&mut buffer).len();
        self.put(&buffer[..encoded]);
    }

    fn grow_byte_buffer_if_needed(&mut self, len: usize) {
        if self.bb_capacity < MAX_BYTE_BUFFER_CAPACITY {
            let new_capacity = (len * 3).min(MAX_BYTE_BUFFER_CAPACITY);
            if new_capacity > self.bb_capacity {
                self.write_bytes();
                self.bb_capacity = new_capacity;
            }
        }
    }

    fn flush_leftover_char(&mut self, cb: &[u16], position: &mut usize, end_of_input: bool) -> Result<(), Malformed> {
        if !self.have_leftover_char && !end_of_input {
            return Ok(());
        }
        let mut lcb = Vec::with_capacity(2);
        if self.have_leftover_char {
            lcb.push(self.leftover_char);
        }
        if *position < cb.len() {
            lcb.push(cb[*position]);
            *position += 1;
        }
        match lcb.as_slice() {
            [] => {}
            [high] if !end_of_input && is_high(*high) => {
                self.leftover_char = *high;
                return Ok(());
            }
            [high, low] if is_high(*high) && is_low(*low) => self.put_code_point(code_point(*high, *low)),
            [high, next] if is_high(*high) && is_high(*next) && !end_of_input => return Err(Malformed::Leftover),
            _ => return Err(Malformed::Leftover),
        }
        self.have_leftover_char = false;
        Ok(())
    }

    fn impl_write(&mut self, cb: &[u16]) -> Result<(), Malformed> {
        let mut position = 0;
        if self.have_leftover_char {
            self.flush_leftover_char(cb, &mut position, false)?;
        }
        self.grow_byte_buffer_if_needed(cb.len() - position);
        while position < cb.len() {
            let c = cb[position];
            if is_high(c) {
                if position + 1 == cb.len() {
                    self.have_leftover_char = true;
                    self.leftover_char = c;
                    return Ok(());
                }
                let next = cb[position + 1];
                if !is_low(next) {
                    return Err(Malformed::Encode);
                }
                self.put_code_point(code_point(c, next));
                position += 2;
            } else if is_low(c) {
                return Err(Malformed::Encode);
            } else {
                self.put_code_point(c as u32);
                position += 1;
            }
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), JavaThrowable> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let mut position = 0;
        if self.flush_leftover_char(&[], &mut position, true).is_err() {
            return Err(JavaThrowable::new(MALFORMED_INPUT, vec![THROW_EXCEPTION, FLUSH_LEFTOVER_CHAR, IMPL_CLOSE, STREAM_ENCODER_CLOSE, OUTPUT_STREAM_WRITER_CLOSE]));
        }
        self.write_bytes();
        Ok(())
    }
}

fn is_high(unit: u16) -> bool {
    (0xD800..0xDC00).contains(&unit)
}

fn is_low(unit: u16) -> bool {
    (0xDC00..0xE000).contains(&unit)
}

fn code_point(high: u16, low: u16) -> u32 {
    0x10000 + (((high as u32) - 0xD800) << 10 | ((low as u32) - 0xDC00))
}

fn malformed(kind: Malformed, caller: &[&'static str]) -> JavaThrowable {
    let mut stack_trace = match kind {
        Malformed::Encode => vec![THROW_EXCEPTION, IMPL_WRITE_ENCODE],
        Malformed::Leftover => vec![THROW_EXCEPTION, FLUSH_LEFTOVER_CHAR, IMPL_WRITE_LEFTOVER],
    };
    stack_trace.extend_from_slice(&[IMPL_WRITE_WRAP, STREAM_ENCODER_WRITE, OUTPUT_STREAM_WRITER_WRITE, FLUSH_BUFFER]);
    stack_trace.extend_from_slice(caller);
    JavaThrowable::new(MALFORMED_INPUT, stack_trace)
}

pub struct BufferedWriter {
    cb: Vec<u16>,
    next_char: usize,
    encoder: StreamEncoder,
    open: bool,
}

impl Default for BufferedWriter {
    fn default() -> BufferedWriter {
        BufferedWriter {
            cb: vec![0; DEFAULT_MAX_BUFFER_SIZE],
            next_char: 0,
            encoder: StreamEncoder { bb: Vec::new(), bb_capacity: INITIAL_BYTE_BUFFER_CAPACITY, have_leftover_char: false, leftover_char: 0, written: Vec::new(), closed: false },
            open: true,
        }
    }
}

impl BufferedWriter {
    fn flush_buffer(&mut self) -> Result<(), Malformed> {
        if self.next_char == 0 {
            return Ok(());
        }
        let chars = self.cb[..self.next_char].to_vec();
        self.encoder.impl_write(&chars)?;
        self.next_char = 0;
        Ok(())
    }

    fn write_units(&mut self, units: &[u16]) -> Result<(), Malformed> {
        let mut b = 0;
        while b < units.len() {
            let d = (DEFAULT_MAX_BUFFER_SIZE - self.next_char).min(units.len() - b);
            self.cb[self.next_char..self.next_char + d].copy_from_slice(&units[b..b + d]);
            b += d;
            self.next_char += d;
            if self.next_char >= DEFAULT_MAX_BUFFER_SIZE {
                self.flush_buffer()?;
            }
        }
        Ok(())
    }

    pub fn write(&mut self, text: &str) -> Result<(), JavaThrowable> {
        let units: Vec<u16> = utf16_units(text).collect();
        self.write_units(&units).map_err(|kind| malformed(kind, &[BUFFERED_WRITER_WRITE, WRITER_WRITE]))
    }

    pub fn print(&mut self, text: &str) -> bool {
        let units: Vec<u16> = utf16_units(text).collect();
        self.write_units(&units).is_ok()
    }

    pub fn flush(&mut self) -> Result<(), JavaThrowable> {
        self.flush_buffer().map_err(|kind| malformed(kind, &[BUFFERED_WRITER_FLUSH]))?;
        self.encoder.write_bytes();
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), JavaThrowable> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        match self.flush_buffer() {
            Ok(()) => self.encoder.close().map_err(|mut close| {
                close.push_frame(BUFFERED_WRITER_CLOSE);
                close
            }),
            Err(kind) => {
                let mut failure = malformed(kind, &[BUFFERED_WRITER_CLOSE_FLUSH]);
                if let Err(mut close) = self.encoder.close() {
                    close.push_frame(BUFFERED_WRITER_CLOSE_AFTER_FAILURE);
                    failure.add_suppressed(close);
                }
                Err(failure)
            }
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.encoder.written
    }

    pub fn take_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.encoder.written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn trace(throwable: &JavaThrowable) -> String {
        let mut out = throwable.description.clone();
        for frame in &throwable.stack_trace {
            out.push_str(" | ");
            out.push_str(frame);
        }
        for suppressed in &throwable.suppressed {
            out.push_str(" || suppressed ");
            out.push_str(&trace(suppressed));
        }
        out
    }

    fn run(mode: &str, writes: &[Vec<u16>]) -> (String, String) {
        let mut writer = BufferedWriter::default();
        let failure = if mode == "2" {
            let mut trouble = false;
            for units in writes {
                trouble |= writer.write_units(units).is_err();
            }
            trouble |= writer.close().is_err();
            (if trouble { "trouble" } else { "ok" }).to_string()
        } else {
            let mut body = Ok(());
            for units in writes {
                body = writer.write_units(units).map_err(|kind| malformed(kind, &[BUFFERED_WRITER_WRITE, WRITER_WRITE]));
                if body.is_err() {
                    break;
                }
            }
            if body.is_ok() && mode == "1" {
                body = writer.flush();
            }
            let result = match (body, writer.close()) {
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(close)) => Err(close),
                (Err(mut primary), close) => {
                    if let Err(close) = close {
                        primary.add_suppressed(close);
                    }
                    Err(primary)
                }
            };
            match result {
                Ok(()) => "ok".to_string(),
                Err(throwable) => trace(&throwable),
            }
        };
        (hex(&writer.into_bytes()), failure)
    }

    #[test]
    fn a_lone_surrogate_in_the_last_chunk_is_written_twice_after_an_explicit_flush() {
        let content: Vec<u16> = "a\nM".encode_utf16().chain([0xDBF6, 0xDC00, 0x4E, 0xD800, 0x4F, 0x0A]).collect();
        let (bytes, failure) = run("1", &[content]);
        assert_eq!(bytes, "610a4df48da0804e610a4df48da0804e");
        assert!(failure.starts_with("java.nio.charset.MalformedInputException: Input length = 1 | "));
        assert!(failure.contains("java.io.BufferedWriter.flush(BufferedWriter.java:305) || suppressed"));
    }

    #[test]
    fn a_high_surrogate_left_over_at_close_loses_the_buffered_bytes() {
        let mut first: Vec<u16> = vec![b'x' as u16; 8191];
        first.push(0xD800);
        let (bytes, failure) = run("0", &[first, vec![b'y' as u16]]);
        assert_eq!(bytes, "");
        assert!(failure.contains("StreamEncoder.implWrite(StreamEncoder.java:274)"));
        assert!(failure.contains("|| suppressed java.nio.charset.MalformedInputException: Input length = 1 | java.base/java.nio.charset.CoderResult.throwException(CoderResult.java:279) | java.base/sun.nio.cs.StreamEncoder.flushLeftoverChar(StreamEncoder.java:258) | java.base/sun.nio.cs.StreamEncoder.implClose(StreamEncoder.java:326)"));
    }

    #[test]
    #[ignore]
    fn buffered_writers_match_the_jdk_oracle() {
        let expected = std::fs::read_to_string(std::env::var("WRITER_ORACLE").expect("WRITER_ORACLE")).unwrap();
        for (index, line) in expected.lines().enumerate() {
            let fields: Vec<&str> = line.split('\t').collect();
            let writes: Vec<Vec<u16>> = fields[1]
                .split(',')
                .filter(|write| !write.is_empty())
                .map(|write| (0..write.len()).step_by(4).map(|i| u16::from_str_radix(&write[i..i + 4], 16).unwrap()).collect())
                .collect();
            let (bytes, failure) = run(fields[0], &writes);
            assert_eq!(bytes, fields[2], "case {index}");
            assert_eq!(failure, fields[3], "case {index}");
        }
    }
}
