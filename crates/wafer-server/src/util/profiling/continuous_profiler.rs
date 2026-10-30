use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use super::active_profiler::{ActiveProfiler, SharedProfiler, lock};
use super::profile_results::ProfileResults;

pub struct ContinuousProfiler {
    tick_count: Arc<AtomicI32>,
    suppress_warnings: bool,
    profiler: Option<SharedProfiler>,
}

impl ContinuousProfiler {
    pub fn new(tick_count: Arc<AtomicI32>, suppress_warnings: bool) -> ContinuousProfiler {
        ContinuousProfiler { tick_count, suppress_warnings, profiler: None }
    }

    pub fn is_enabled(&self) -> bool {
        self.profiler.is_some()
    }

    pub fn disable(&mut self) {
        self.profiler = None;
    }

    pub fn enable(&mut self) {
        let tick_count = self.tick_count.clone();
        self.profiler = Some(Arc::new(Mutex::new(ActiveProfiler::new(Box::new(move || tick_count.load(Ordering::Relaxed)), self.suppress_warnings))));
    }

    pub fn get_filler(&self) -> Option<SharedProfiler> {
        self.profiler.clone()
    }

    pub fn get_results(&self) -> ProfileResults {
        match &self.profiler {
            Some(profiler) => lock(profiler).get_results(),
            None => ProfileResults::Empty,
        }
    }
}
