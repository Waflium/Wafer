use wafer_protocol::{Writer, stream_codec};

use crate::network::chat::Component;

pub fn write(w: &mut Writer, content: &Component, overlay: bool) {
    w.layer(&[stream_codec::composite(2, 1)], |w| content.write(w));
    w.bool(overlay);
}
