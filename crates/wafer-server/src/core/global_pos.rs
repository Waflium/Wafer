use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct GlobalPos {
    pub dimension: String,
    pub pos: BlockPos,
}

impl GlobalPos {
    pub fn of(dimension: &str, pos: BlockPos) -> GlobalPos {
        GlobalPos { dimension: dimension.to_string(), pos }
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

    pub fn from_nbt(nbt: &Nbt) -> Option<GlobalPos> {
        let dimension = nbt.get("dimension")?.as_str()?.to_string();
        let pos = match nbt.get("pos")? {
            Nbt::IntArray(v) if v.len() == 3 => BlockPos::new(v[0], v[1], v[2]),
            _ => return None,
        };
        Some(GlobalPos { dimension, pos })
    }
}
