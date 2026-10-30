use super::rfr_profiler::RfrProfiler;

pub fn instance() -> &'static RfrProfiler {
    RfrProfiler::get_instance()
}
