use wafer_protocol::{Writer, stream_codec};

use crate::network::chat::Component;
use crate::network::chat_type::Bound;

pub fn write(w: &mut Writer, message: &Component, chat_type: &Bound) {
    w.layer(&[stream_codec::composite(2, 1)], |w| message.write(w));
    w.layer(&[stream_codec::composite(2, 2)], |w| chat_type.write(w));
}
