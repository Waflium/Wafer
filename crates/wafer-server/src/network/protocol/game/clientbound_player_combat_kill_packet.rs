use wafer_protocol::{Writer, stream_codec};

use crate::network::chat::Component;
use crate::world::entity::EntityId;

pub fn write(w: &mut Writer, player_id: EntityId, message: &Component) {
    w.varint(player_id);
    w.layer(&[stream_codec::composite(2, 2)], |w| message.write(w));
}
