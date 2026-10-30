use std::convert::Infallible;

const FLIGHT_RECORDER_CONFIG: &str = "/flightrecorder-config.jfc";

pub struct JfrProfiler;

pub static INSTANCE: JfrProfiler = JfrProfiler;

impl JfrProfiler {
    pub fn start(&self) -> bool {
        warn!("Could not find default flight recorder config at {FLIGHT_RECORDER_CONFIG}");
        false
    }

    pub fn stop(&self) -> Result<Infallible, String> {
        Err("Not currently profiling".to_string())
    }
}
