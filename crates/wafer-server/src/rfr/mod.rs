pub mod allocation;
pub mod chunk;
pub mod consumer;
pub mod event_type;
pub mod io;
pub mod recording;
pub mod runtime;
pub mod thread;

use std::sync::OnceLock;
use std::time::Instant;

pub use event_type::{ContentType, EventType, FieldType, Value, ValueDescriptor};
pub use recording::Recording;

pub const TICKS_PER_SECOND: i64 = 1_000_000_000;

static START: OnceLock<Instant> = OnceLock::new();

pub fn ticks() -> i64 {
    START.get_or_init(Instant::now).elapsed().as_nanos() as i64 + 1
}
