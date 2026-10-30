use wafer_protocol::nbt::Nbt;

use super::codec::{self, DataResult};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Brightness {
    pub block: i32,
    pub sky: i32,
}

fn light_value(tag: &Nbt) -> DataResult<i32> {
    let value = codec::int(tag)?;
    if (0..=15).contains(&value) { Ok(value) } else { Err(format!("Value must be within range [0;15]: {value}")) }
}

impl Brightness {
    pub fn decode(tag: &Nbt) -> DataResult<Brightness> {
        Ok(Brightness { block: codec::decode_field(tag, "block", light_value)?, sky: codec::decode_field(tag, "sky", light_value)? })
    }

    pub fn encode(self) -> Nbt {
        Nbt::Compound(vec![("block".into(), Nbt::Int(self.block)), ("sky".into(), Nbt::Int(self.sky))].into())
    }

    pub fn pack(self) -> i32 {
        self.block << 4 | self.sky << 20
    }

    pub fn unpack(packed: i32) -> Brightness {
        Brightness { block: packed >> 4 & 15, sky: packed >> 20 & 15 }
    }
}
