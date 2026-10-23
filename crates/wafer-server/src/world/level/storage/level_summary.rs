use crate::util::datafix::data_fixers::get_file_fixer;

use super::data_version::DataVersion;
use super::level_version::LevelVersion;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackupStatus {
    None,
    Downgrade,
    UpgradeToSnapshot,
    FileFixingRequired,
}

impl BackupStatus {
    pub fn should_backup(self) -> bool {
        self != BackupStatus::None
    }

    pub fn is_severe(self) -> bool {
        self == BackupStatus::Downgrade
    }

    pub fn get_translation_key(self) -> &'static str {
        match self {
            BackupStatus::None => "",
            BackupStatus::Downgrade => "downgrade",
            BackupStatus::UpgradeToSnapshot => "snapshot",
            BackupStatus::FileFixingRequired => "file_fixing_required",
        }
    }
}

pub struct LevelSummary {
    level_version: LevelVersion,
    level_id: String,
    requires_manual_conversion: bool,
    requires_file_fixing: bool,
    locked: bool,
}

impl LevelSummary {
    pub fn new(level_version: LevelVersion, level_id: String, requires_manual_conversion: bool, requires_file_fixing: bool, locked: bool) -> LevelSummary {
        LevelSummary { level_version, level_id, requires_manual_conversion, requires_file_fixing, locked }
    }

    pub fn get_level_id(&self) -> &str {
        &self.level_id
    }

    pub fn requires_manual_conversion(&self) -> bool {
        self.requires_manual_conversion
    }

    pub fn requires_file_fixing(&self) -> bool {
        self.requires_file_fixing
    }

    pub fn get_last_played(&self) -> i64 {
        self.level_version.last_played()
    }

    pub fn level_version(&self) -> &LevelVersion {
        &self.level_version
    }

    pub fn should_backup(&self) -> bool {
        self.backup_status().should_backup()
    }

    pub fn is_downgrade(&self) -> bool {
        self.backup_status() == BackupStatus::Downgrade
    }

    pub fn backup_status(&self) -> BackupStatus {
        let current_version_number = wafer_data::WORLD_VERSION;
        let level_version_number = self.level_version.minecraft_version().version();
        if get_file_fixer().requires_file_fixing(level_version_number) {
            BackupStatus::FileFixingRequired
        } else if level_version_number > current_version_number {
            BackupStatus::Downgrade
        } else {
            BackupStatus::None
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn is_disabled(&self) -> bool {
        if !self.is_locked() && !self.requires_manual_conversion() { !self.is_compatible() } else { true }
    }

    pub fn is_compatible(&self) -> bool {
        DataVersion::new(wafer_data::WORLD_VERSION, "main").is_compatible(self.level_version.minecraft_version())
    }
}
