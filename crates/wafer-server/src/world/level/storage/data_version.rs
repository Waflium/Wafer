pub const MAIN_SERIES: &str = "main";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataVersion {
    pub version: i32,
    pub series: String,
}

impl DataVersion {
    pub fn new(version: i32, series: &str) -> DataVersion {
        DataVersion { version, series: series.to_string() }
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn series(&self) -> &str {
        &self.series
    }

    pub fn is_side_series(&self) -> bool {
        self.series != MAIN_SERIES
    }

    pub fn is_compatible(&self, other: &DataVersion) -> bool {
        self.series == other.series
    }
}
