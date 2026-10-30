mod buf;
pub mod fault;
mod frame;
pub mod stream_codec;
pub use wafer_core::java_string;
pub mod lazily_parsed_number;
pub mod nbt;

pub use buf::{DecodeError, Reader, Writer};
pub use fault::{Fault, Frame};
pub use frame::{ALWAYS_ENCODABLE_LEN, Codec, read_frame, write_frame};
