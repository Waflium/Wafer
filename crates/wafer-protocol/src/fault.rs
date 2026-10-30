use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub description: String,
    pub frames: Vec<&'static str>,
    pub cause: Option<Box<Fault>>,
}

impl Fault {
    pub fn new(description: impl Into<String>, frames: &[Frame]) -> Fault {
        Fault { description: description.into(), frames: frames.iter().map(|frame| frame.resolve()).collect(), cause: None }
    }

    pub fn with_cause(mut self, cause: Fault) -> Fault {
        self.cause = Some(Box::new(cause));
        self
    }

    pub fn push_frames(&mut self, frames: &[Frame]) {
        for frame in frames {
            self.push_frame(frame.resolve());
        }
    }

    pub fn push_frame(&mut self, frame: &'static str) {
        self.frames.push(frame);
        if let Some(cause) = self.cause.as_deref_mut() {
            cause.push_frame(frame);
        }
    }

    pub fn root(&self) -> &Fault {
        self.cause.as_deref().map_or(self, Fault::root)
    }

    pub fn has_frames(&self) -> bool {
        !self.frames.is_empty() || self.cause.as_deref().is_some_and(Fault::has_frames)
    }

    pub fn write_to(&self, out: &mut Vec<u8>) {
        put_str(out, &self.description);
        out.extend_from_slice(&(self.frames.len() as u32).to_be_bytes());
        for frame in &self.frames {
            put_str(out, frame);
        }
        out.push(self.cause.is_some() as u8);
        if let Some(cause) = &self.cause {
            cause.write_to(out);
        }
    }

    pub fn read_from(input: &mut &[u8]) -> Option<Fault> {
        let description = take_str(input)?.to_string();
        let count = u32::from_be_bytes(take(input, 4)?.try_into().ok()?);
        let mut frames = Vec::new();
        for _ in 0..count {
            frames.push(intern(take_str(input)?));
        }
        let cause = match take(input, 1)?[0] {
            0 => None,
            _ => Some(Box::new(Fault::read_from(input)?)),
        };
        Some(Fault { description, frames, cause })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Frame {
    Fixed(&'static str),
    Composite { size: u8, field: u8 },
}

const COMPOSITE_FIRST_LINES: [u16; 14] = [142, 162, 185, 211, 240, 272, 307, 345, 386, 430, 477, 527, 580, 636];

impl Frame {
    pub const fn composite(size: u8, field: u8) -> Frame {
        Frame::Composite { size, field }
    }

    pub fn resolve(self) -> &'static str {
        match self {
            Frame::Fixed(text) => text,
            Frame::Composite { size, field } => {
                let line = COMPOSITE_FIRST_LINES[size as usize - 1] + field as u16 - 1;
                intern(&format!("net.minecraft.network.codec.StreamCodec${}.encode(StreamCodec.java:{line})", size + 6))
            }
        }
    }
}

fn intern(text: &str) -> &'static str {
    static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut interned = INTERNED.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = interned.get(text) {
        return existing;
    }
    let leaked: &'static str = Box::leak(text.to_string().into_boxed_str());
    interned.insert(leaked);
    leaked
}

fn put_str(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

fn take<'a>(input: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    let (head, tail) = input.split_at_checked(n)?;
    *input = tail;
    Some(head)
}

fn take_str<'a>(input: &mut &'a [u8]) -> Option<&'a str> {
    let len = u32::from_be_bytes(take(input, 4)?.try_into().ok()?) as usize;
    std::str::from_utf8(take(input, len)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_frames_follow_stream_codec_lines() {
        assert_eq!(Frame::composite(1, 1).resolve(), "net.minecraft.network.codec.StreamCodec$7.encode(StreamCodec.java:142)");
        assert_eq!(Frame::composite(2, 2).resolve(), "net.minecraft.network.codec.StreamCodec$8.encode(StreamCodec.java:163)");
        assert_eq!(Frame::composite(8, 6).resolve(), "net.minecraft.network.codec.StreamCodec$14.encode(StreamCodec.java:350)");
    }

    #[test]
    fn pushed_frames_reach_every_cause_and_survive_serialisation() {
        let mut fault = Fault::new("a: outer", &[Frame::Fixed("x.A.a(A.java:1)")]).with_cause(Fault::new("b: inner", &[Frame::Fixed("x.B.b(B.java:2)")]));
        fault.push_frame("x.C.c(C.java:3)");
        assert_eq!(fault.frames, ["x.A.a(A.java:1)", "x.C.c(C.java:3)"]);
        assert_eq!(fault.root().frames, ["x.B.b(B.java:2)", "x.C.c(C.java:3)"]);
        let mut bytes = Vec::new();
        fault.write_to(&mut bytes);
        assert_eq!(Fault::read_from(&mut &bytes[..]), Some(fault));
    }
}
