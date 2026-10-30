pub mod active_profiler;
pub mod continuous_profiler;
pub mod metrics;
pub mod profile_results;
pub mod profiler;
pub mod rfr;

pub fn get_nanos() -> i64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_nanos() as i64
}
