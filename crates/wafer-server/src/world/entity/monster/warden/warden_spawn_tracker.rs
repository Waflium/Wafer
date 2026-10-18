use wafer_core::mth;
use wafer_protocol::nbt::Nbt;

use crate::world::level::block::entity::int_value;

pub const MAX_WARNING_LEVEL: i32 = 4;
const DECREASE_WARNING_LEVEL_EVERY_INTERVAL: i32 = 12000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WardenSpawnTracker {
    ticks_since_last_warning: i32,
    warning_level: i32,
    cooldown_ticks: i32,
}

impl WardenSpawnTracker {
    pub fn new(ticks_since_last_warning: i32, warning_level: i32, cooldown_ticks: i32) -> WardenSpawnTracker {
        WardenSpawnTracker { ticks_since_last_warning, warning_level, cooldown_ticks }
    }

    pub fn tick(&mut self) {
        if self.ticks_since_last_warning >= DECREASE_WARNING_LEVEL_EVERY_INTERVAL {
            self.decrease_warning_level();
            self.ticks_since_last_warning = 0;
        } else {
            self.ticks_since_last_warning += 1;
        }
        if self.cooldown_ticks > 0 {
            self.cooldown_ticks -= 1;
        }
    }

    fn decrease_warning_level(&mut self) {
        self.set_warning_level(self.get_warning_level() - 1);
    }

    pub fn set_warning_level(&mut self, warning_level: i32) {
        self.warning_level = mth::clamp_i(warning_level, 0, MAX_WARNING_LEVEL);
    }

    pub fn get_warning_level(&self) -> i32 {
        self.warning_level
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("ticks_since_last_warning".into(), Nbt::Int(self.ticks_since_last_warning)),
            ("warning_level".into(), Nbt::Int(self.warning_level)),
            ("cooldown_ticks".into(), Nbt::Int(self.cooldown_ticks)),
        ])
    }

    pub fn from_nbt(input: &Nbt) -> Option<WardenSpawnTracker> {
        let Nbt::Compound(_) = input else { return None };
        let field = |key: &str| match input.get(key) {
            Some(tag) => int_value(tag).filter(|&value| value >= 0),
            None => Some(0),
        };
        Some(WardenSpawnTracker::new(field("ticks_since_last_warning")?, field("warning_level")?, field("cooldown_ticks")?))
    }
}
