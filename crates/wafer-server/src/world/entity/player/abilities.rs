use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult, Record};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Abilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub mayfly: bool,
    pub instabuild: bool,
    pub may_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl Default for Abilities {
    fn default() -> Abilities {
        Abilities { invulnerable: false, flying: false, mayfly: false, instabuild: false, may_build: true, flying_speed: 0.05, walking_speed: 0.1 }
    }
}

impl Abilities {
    pub fn write(&self, w: &mut Writer) {
        let flags = self.invulnerable as u8 | (self.flying as u8) << 1 | (self.mayfly as u8) << 2 | (self.instabuild as u8) << 3;
        w.u8(flags);
        w.f32(self.flying_speed);
        w.f32(self.walking_speed);
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("invulnerable".into(), Nbt::Byte(self.invulnerable as i8)),
            ("flying".into(), Nbt::Byte(self.flying as i8)),
            ("mayfly".into(), Nbt::Byte(self.mayfly as i8)),
            ("instabuild".into(), Nbt::Byte(self.instabuild as i8)),
            ("mayBuild".into(), Nbt::Byte(self.may_build as i8)),
            ("flySpeed".into(), Nbt::Float(self.flying_speed)),
            ("walkSpeed".into(), Nbt::Float(self.walking_speed)),
        ])
    }

    pub fn decode(nbt: &Nbt) -> DecodeResult<Abilities> {
        if let Err(error) = codec::map(nbt) {
            return DecodeResult::error(error);
        }
        let d = Abilities::default();
        let mut record = Record::default();
        let invulnerable = record.with_default(nbt, "invulnerable", d.invulnerable, codec::bool_value);
        let flying = record.with_default(nbt, "flying", d.flying, codec::bool_value);
        let mayfly = record.with_default(nbt, "mayfly", d.mayfly, codec::bool_value);
        let instabuild = record.with_default(nbt, "instabuild", d.instabuild, codec::bool_value);
        let may_build = record.with_default(nbt, "mayBuild", d.may_build, codec::bool_value);
        let flying_speed = record.with_default(nbt, "flySpeed", d.flying_speed, codec::float);
        let walking_speed = record.with_default(nbt, "walkSpeed", d.walking_speed, codec::float);
        record.build(|| Some(Abilities { invulnerable, flying, mayfly, instabuild, may_build, flying_speed, walking_speed }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_keeps_defaults_for_missing_fields() {
        let nbt = crate::nbt::parse_fully("{flying:1b,walkSpeed:0.25f}").unwrap();
        let decoded = Abilities::decode(&nbt);
        assert!(decoded.error.is_none());
        let abilities = decoded.value.unwrap();
        assert!(abilities.flying);
        assert_eq!(abilities.walking_speed, 0.25);
        assert_eq!(abilities.flying_speed, Abilities::default().flying_speed);
    }

    #[test]
    fn decode_fails_for_a_bad_field() {
        let nbt = crate::nbt::parse_fully("{flying:\"x\"}").unwrap();
        let decoded = Abilities::decode(&nbt);
        assert!(decoded.error.is_some());
    }
}
