use std::cell::Cell;

use wafer_protocol::nbt::Nbt;

pub struct WeatherDataCell {
    data: Cell<WeatherData>,
    dirty: Cell<bool>,
}

impl Default for WeatherDataCell {
    fn default() -> WeatherDataCell {
        WeatherDataCell::new(WeatherData::default(), true)
    }
}

impl WeatherDataCell {
    pub fn new(data: WeatherData, dirty: bool) -> WeatherDataCell {
        WeatherDataCell { data: Cell::new(data), dirty: Cell::new(dirty) }
    }

    pub fn get(&self) -> WeatherData {
        self.data.get()
    }

    pub fn set(&self, data: WeatherData) {
        self.data.set(data);
        self.dirty.set(true);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.dirty.set(dirty);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeatherData {
    pub clear_weather_time: i32,
    pub rain_time: i32,
    pub thunder_time: i32,
    pub raining: bool,
    pub thundering: bool,
}

impl WeatherData {
    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("clear_weather_time".into(), Nbt::Int(self.clear_weather_time)),
            ("rain_time".into(), Nbt::Int(self.rain_time)),
            ("thunder_time".into(), Nbt::Int(self.thunder_time)),
            ("raining".into(), Nbt::Byte(self.raining as i8)),
            ("thundering".into(), Nbt::Byte(self.thundering as i8)),
        ])
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<WeatherData> {
        let int = |key: &str| nbt.get(key)?.as_i64().map(|v| v as i32);
        let bool = |key: &str| nbt.get(key)?.as_i64().map(|v| v != 0);
        Some(WeatherData {
            clear_weather_time: int("clear_weather_time")?,
            rain_time: int("rain_time")?,
            thunder_time: int("thunder_time")?,
            raining: bool("raining")?,
            thundering: bool("thundering")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::random::LegacyRandomSource;

    use crate::server::level::testing;

    #[test]
    fn weather_cycle_follows_vanilla_draws() {
        let mut level = testing::level();
        level.random = LegacyRandomSource::new(42);
        level.tick();
        let weather = level.weather.get();
        assert_eq!((weather.thunder_time, weather.rain_time), (33830, 140065));
        assert!(!weather.raining);

        level.weather.set(super::WeatherData { rain_time: 1, ..weather });
        level.tick();
        assert!(level.weather.get().raining);
        let mut ticks = 1;
        while !level.is_raining() {
            level.tick();
            ticks += 1;
        }
        assert_eq!(ticks, 20);
    }
}
