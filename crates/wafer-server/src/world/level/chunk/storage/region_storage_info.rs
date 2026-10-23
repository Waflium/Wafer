#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionStorageInfo {
    pub level: String,
    pub dimension: String,
    pub type_: String,
}

impl RegionStorageInfo {
    pub fn new(level: &str, dimension: &str, type_: &str) -> RegionStorageInfo {
        RegionStorageInfo { level: level.to_string(), dimension: dimension.to_string(), type_: type_.to_string() }
    }

    pub fn with_type_suffix(&self, suffix: &str) -> RegionStorageInfo {
        RegionStorageInfo { level: self.level.clone(), dimension: self.dimension.clone(), type_: format!("{}{suffix}", self.type_) }
    }
}
