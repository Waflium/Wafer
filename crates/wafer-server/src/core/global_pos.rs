use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use crate::util::{codec, extra_codecs};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct GlobalPos {
    pub dimension: String,
    pub pos: BlockPos,
}

impl GlobalPos {
    pub fn of(dimension: &str, pos: BlockPos) -> GlobalPos {
        GlobalPos { dimension: dimension.to_string(), pos }
    }

    pub fn is_close_enough(&self, dimension: &str, pos: BlockPos, max_distance: i32) -> bool {
        self.dimension == dimension && self.pos.dist_chessboard(pos) <= max_distance
    }

    pub fn dimension(&self) -> &str {
        &self.dimension
    }

    pub fn pos(&self) -> BlockPos {
        self.pos
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("dimension".into(), Nbt::String(self.dimension.clone())),
            ("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z])),
        ])
    }

    pub fn decode(nbt: &Nbt) -> Result<GlobalPos, String> {
        codec::map(nbt)?;
        let mut record = codec::Record::default();
        let dimension = record.required(nbt, "dimension", |dimension| codec::identifier(dimension));
        let pos = record.required(nbt, "pos", extra_codecs::block_pos);
        record.build(|| Some(GlobalPos { dimension: dimension?, pos: pos? })).into_result()
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<GlobalPos> {
        let dimension = nbt.get("dimension")?.as_str()?.to_string();
        let pos = match nbt.get("pos")? {
            Nbt::IntArray(v) if v.len() == 3 => BlockPos::new(v[0], v[1], v[2]),
            _ => return None,
        };
        Some(GlobalPos { dimension, pos })
    }
}
