use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::metrics::metric_category::MetricCategory;
use super::profile_results::{FilledProfileResults, ProfileResults, demangle_path};
use crate::util::fastutil::ObjectOpenHashMap;

const WARNING_TIME_NANOS: i64 = 100_000_000;

pub type SharedProfiler = Arc<Mutex<ActiveProfiler>>;

pub fn lock(profiler: &SharedProfiler) -> MutexGuard<'_, ActiveProfiler> {
    profiler.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone)]
pub struct PathEntry {
    pub max_duration: i64,
    pub min_duration: i64,
    pub accumulated_duration: i64,
    pub count: i64,
    pub counters: ObjectOpenHashMap<i64>,
}

impl Default for PathEntry {
    fn default() -> PathEntry {
        PathEntry { max_duration: i64::MIN, min_duration: i64::MAX, accumulated_duration: 0, count: 0, counters: ObjectOpenHashMap::default() }
    }
}

pub struct ActiveProfiler {
    paths: Vec<String>,
    start_times: Vec<i64>,
    entries: HashMap<String, PathEntry>,
    get_tick_time: Box<dyn Fn() -> i32 + Send>,
    start_time_nano: i64,
    start_time_ticks: i32,
    path: String,
    started: bool,
    suppress_warnings: bool,
    charted_paths: Vec<(String, MetricCategory)>,
}

impl ActiveProfiler {
    pub fn new(get_tick_time: Box<dyn Fn() -> i32 + Send>, suppress_warnings: bool) -> ActiveProfiler {
        ActiveProfiler {
            paths: Vec::new(),
            start_times: Vec::new(),
            entries: HashMap::new(),
            start_time_nano: super::get_nanos(),
            start_time_ticks: get_tick_time(),
            get_tick_time,
            path: String::new(),
            started: false,
            suppress_warnings,
            charted_paths: Vec::new(),
        }
    }

    pub fn start_tick(&mut self) {
        if self.started {
            error!("Profiler tick already started - missing endTick()?");
        } else {
            self.started = true;
            self.path.clear();
            self.paths.clear();
            self.push("root");
        }
    }

    pub fn end_tick(&mut self) {
        if !self.started {
            error!("Profiler tick already ended - missing startTick()?");
        } else {
            self.pop();
            self.started = false;
            if !self.path.is_empty() {
                error!("Profiler tick ended before path was fully popped (remainder: '{}'). Mismatched push/pop?", demangle_path(&self.path));
            }
        }
    }

    pub fn push(&mut self, name: &str) {
        if !self.started {
            error!("Cannot push '{name}' to profiler if profiler tick hasn't started - missing startTick()?");
        } else {
            if !self.path.is_empty() {
                self.path.push('\u{1e}');
            }
            self.path.push_str(name);
            self.paths.push(self.path.clone());
            self.start_times.push(super::get_nanos());
        }
    }

    pub fn mark_for_charting(&mut self, category: MetricCategory) {
        let entry = (self.path.clone(), category);
        if !self.charted_paths.contains(&entry) {
            self.charted_paths.push(entry);
        }
    }

    pub fn pop(&mut self) {
        if !self.started {
            error!("Cannot pop from profiler if profiler tick hasn't started - missing startTick()?");
        } else if self.start_times.is_empty() {
            error!("Tried to pop one too many times! Mismatched push() and pop()?");
        } else {
            let end_time = super::get_nanos();
            let start_time = self.start_times.pop().unwrap_or_default();
            self.paths.pop();
            let time = end_time - start_time;
            let entry = self.entries.entry(self.path.clone()).or_default();
            entry.accumulated_duration += time;
            entry.count += 1;
            entry.max_duration = entry.max_duration.max(time);
            entry.min_duration = entry.min_duration.min(time);
            if time > WARNING_TIME_NANOS && !self.suppress_warnings {
                warn!("Something's taking too long! '{}' took approximately {} ms", demangle_path(&self.path), crate::nbt::java_double_to_string(time as f64 / 1000000.0));
            }
            self.path = self.paths.last().cloned().unwrap_or_default();
        }
    }

    pub fn pop_push(&mut self, name: &str) {
        self.pop();
        self.push(name);
    }

    pub fn increment_counter(&mut self, name: &str, amount: i32) {
        let entry = self.entries.entry(self.path.clone()).or_default();
        *entry.counters.entry_or_insert_with(name, || 0) += i64::from(amount);
    }

    pub fn get_results(&self) -> ProfileResults {
        ProfileResults::Filled(FilledProfileResults::new(self.entries.clone(), self.start_time_nano, self.start_time_ticks, super::get_nanos(), (self.get_tick_time)()))
    }

    pub fn get_entry(&self, path: &str) -> Option<&PathEntry> {
        self.entries.get(path)
    }

    pub fn get_charted_paths(&self) -> &[(String, MetricCategory)] {
        &self.charted_paths
    }
}
