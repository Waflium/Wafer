mod buf;
mod frame;
pub use wafer_core::java_string;
pub mod lazily_parsed_number;
pub mod nbt;

pub use buf::{DecodeError, Reader, Writer};
pub use frame::{ALWAYS_ENCODABLE_LEN, Codec, read_frame, write_frame};
