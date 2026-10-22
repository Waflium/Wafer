use wafer_protocol::nbt::Nbt;

use super::vibration_info::VibrationInfo;
use super::vibration_system;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct VibrationSelector {
    current_vibration_data: Option<(VibrationInfo, i64)>,
}

impl VibrationSelector {
    pub fn add_candidate(&mut self, new_vibration: VibrationInfo, tick_time: i64) {
        if self.should_replace_vibration(&new_vibration, tick_time) {
            self.current_vibration_data = Some((new_vibration, tick_time));
        }
    }

    fn should_replace_vibration(&self, new_vibration: &VibrationInfo, tick_time: i64) -> bool {
        let Some((previous_vibration, previous_tick)) = &self.current_vibration_data else { return true };
        if tick_time != *previous_tick {
            return false;
        }
        if new_vibration.distance < previous_vibration.distance {
            true
        } else if new_vibration.distance > previous_vibration.distance {
            false
        } else {
            vibration_system::get_game_event_frequency(new_vibration.game_event) > vibration_system::get_game_event_frequency(previous_vibration.game_event)
        }
    }

    pub fn chosen_candidate(&self, time: i64) -> Option<VibrationInfo> {
        self.current_vibration_data.filter(|(_, tick)| *tick < time).map(|(event, _)| event)
    }

    pub fn start_over(&mut self) {
        self.current_vibration_data = None;
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = Vec::new();
        if let Some((event, _)) = &self.current_vibration_data {
            fields.push(("event".to_string(), event.to_nbt()));
        }
        fields.push(("tick".into(), Nbt::Long(self.current_vibration_data.map_or(-1, |(_, tick)| tick))));
        Nbt::Compound(fields)
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<VibrationSelector> {
        let tick = nbt.get("tick")?.as_i64()?;
        let event = nbt.get("event").and_then(VibrationInfo::from_nbt);
        Some(VibrationSelector { current_vibration_data: event.map(|event| (event, tick)) })
    }
}
