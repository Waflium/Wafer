use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::villager_profession::ProfessionId;
use super::villager_type::VillagerType;

pub const DEFAULT_TYPE: VillagerType = VillagerType::PLAINS;
pub const MIN_VILLAGER_LEVEL: i32 = 1;
pub const MAX_VILLAGER_LEVEL: i32 = 5;
const NEXT_LEVEL_XP_THRESHOLDS: [i32; 5] = [0, 10, 70, 150, 250];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VillagerData {
    pub ty: VillagerType,
    pub profession: ProfessionId,
    pub level: i32,
}

impl VillagerData {
    pub fn new(ty: VillagerType, profession: ProfessionId, level: i32) -> VillagerData {
        VillagerData { ty, profession, level: level.max(1) }
    }

    pub fn with_type(self, ty: VillagerType) -> VillagerData {
        VillagerData::new(ty, self.profession, self.level)
    }

    pub fn with_profession(self, profession: ProfessionId) -> VillagerData {
        VillagerData::new(self.ty, profession, self.level)
    }

    pub fn with_level(self, level: i32) -> VillagerData {
        VillagerData::new(self.ty, self.profession, level)
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("level".into(), Nbt::Int(self.level)),
            ("profession".into(), Nbt::String(self.profession.name().into())),
            ("type".into(), Nbt::String(self.ty.name().into())),
        ].into())
    }

    pub fn decode(tag: &Nbt) -> Result<VillagerData, String> {
        crate::util::codec::map(tag)?;
        let ty = tag.get("type").and_then(Nbt::as_str).and_then(|name| VillagerType::by_name(&crate::commands::arguments::parse_identifier(name)?)).unwrap_or(DEFAULT_TYPE);
        let profession = tag.get("profession").and_then(Nbt::as_str).and_then(|name| ProfessionId::by_name(&crate::commands::arguments::parse_identifier(name)?)).unwrap_or(ProfessionId::NONE);
        let level = match tag.get("level") {
            Some(level) => crate::util::codec::int(level)?,
            None => 1,
        };
        Ok(VillagerData::new(ty, profession, level))
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.ty.0);
        w.varint(self.profession.0);
        w.varint(self.level);
    }
}

pub fn get_max_xp_per_level(level: i32) -> i32 {
    if can_level_up(level) { NEXT_LEVEL_XP_THRESHOLDS[level as usize] } else { 0 }
}

pub fn can_level_up(current_level: i32) -> bool {
    (MIN_VILLAGER_LEVEL..MAX_VILLAGER_LEVEL).contains(&current_level)
}
