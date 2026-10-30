use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Writer, stream_codec};

pub fn write(w: &mut Writer, pos: BlockPos, block_entity_type: i32, tag: &Nbt) {
    w.position(pos.x, pos.y, pos.z);
    w.varint(block_entity_type);
    w.layer(&[stream_codec::composite(3, 3)], |w| w.layer(stream_codec::MAP, |w| w.layer(stream_codec::TAG_CODEC, |w| tag.write(w))));
}
