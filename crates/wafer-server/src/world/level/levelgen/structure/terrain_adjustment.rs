#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainAdjustment {
    None,
    Bury,
    BeardThin,
    BeardBox,
    Encapsulate,
}

impl TerrainAdjustment {
    pub const NAMES: &'static [&'static str] = &["none", "bury", "beard_thin", "beard_box", "encapsulate"];

    pub fn by_name(name: &str) -> Option<TerrainAdjustment> {
        Some(match name {
            "none" => TerrainAdjustment::None,
            "bury" => TerrainAdjustment::Bury,
            "beard_thin" => TerrainAdjustment::BeardThin,
            "beard_box" => TerrainAdjustment::BeardBox,
            "encapsulate" => TerrainAdjustment::Encapsulate,
            _ => return None,
        })
    }
}
