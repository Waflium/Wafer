use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;

#[derive(Clone, PartialEq, Debug)]
pub struct MapFrame {
    pub pos: BlockPos,
    pub rotation: i32,
    pub entity_id: i32,
}

impl MapFrame {
    pub fn decode(tag: &Nbt) -> DecodeResult<MapFrame> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let pos = record.required(tag, "pos", extra_codecs::block_pos);
        let rotation = record.required(tag, "rotation", codec::int);
        let entity_id = record.required(tag, "entity_id", codec::int);
        record.build(|| Some(MapFrame { pos: pos?, rotation: rotation?, entity_id: entity_id? }))
    }

    pub fn encode(&self) -> Nbt {
        Nbt::Compound(vec![
            ("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z])),
            ("rotation".into(), Nbt::Int(self.rotation)),
            ("entity_id".into(), Nbt::Int(self.entity_id)),
        ])
    }

    pub fn get_id(&self) -> String {
        frame_id(self.pos)
    }
}

pub fn frame_id(pos: BlockPos) -> String {
    format!("frame-{},{},{}", pos.x, pos.y, pos.z)
}
