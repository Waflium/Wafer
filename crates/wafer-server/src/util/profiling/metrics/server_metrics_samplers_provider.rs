use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use super::metric_category::MetricCategory;
use super::metric_sampler::{MetricSampler, value_increased_by_percentage};
use super::metrics_registry::{ProfilerMeasured, get_registered_samplers};
use crate::oshi::OshiResult;
use crate::oshi::hardware::central_processor::{CentralProcessor, TICK_TYPE_COUNT};
use crate::oshi::system_info::SystemInfo;
use crate::util::fastutil::ObjectOpenHashMap;
use crate::util::profiling::active_profiler::{SharedProfiler, lock};

pub type ProfilerSupplier = Arc<dyn Fn() -> Option<SharedProfiler> + Send + Sync>;

const NANOSECONDS_PER_MILLISECOND: f64 = 1_000_000.0;

pub struct ProfilerSamplerAdapter {
    previously_found_sampler_names: HashSet<String>,
}

impl ProfilerSamplerAdapter {
    pub fn new() -> ProfilerSamplerAdapter {
        ProfilerSamplerAdapter { previously_found_sampler_names: HashSet::new() }
    }

    pub fn new_samplers_found_in_profiler(&mut self, profiler: &ProfilerSupplier) -> Vec<MetricSampler> {
        let charted_paths: Vec<(String, MetricCategory)> = match profiler() {
            Some(active) => lock(&active).get_charted_paths().to_vec(),
            None => Vec::new(),
        };
        let mut new_samplers = Vec::new();
        for (path, category) in charted_paths {
            if self.previously_found_sampler_names.insert(path.clone()) {
                new_samplers.push(sampler_for_profiling_path(profiler.clone(), &path, category));
            }
        }
        new_samplers
    }
}

fn sampler_for_profiling_path(profiler: ProfilerSupplier, profiler_path: &str, category: MetricCategory) -> MetricSampler {
    let path = profiler_path.to_string();
    MetricSampler::create(profiler_path, category, move || {
        let entry_max_duration = profiler().and_then(|active| lock(&active).get_entry(&path).map(|entry| entry.max_duration));
        entry_max_duration.map_or(0.0, |max_duration| max_duration as f64 / NANOSECONDS_PER_MILLISECOND)
    })
}

pub struct ServerMetricsSamplersProvider {
    samplers: ObjectOpenHashMap<MetricSampler>,
    sampler_factory: ProfilerSamplerAdapter,
}

impl ServerMetricsSamplersProvider {
    pub fn new(wall_time_source: fn() -> i64, is_dedicated_server: bool, measured: Vec<ProfilerMeasured>) -> ServerMetricsSamplersProvider {
        let samplers = if is_dedicated_server { runtime_independent_samplers(measured) } else { Vec::new() };
        ServerMetricsSamplersProvider::with_samplers(tick_time_sampler(wall_time_source), samplers)
    }

    pub fn with_samplers(tick_time: MetricSampler, samplers: Vec<MetricSampler>) -> ServerMetricsSamplersProvider {
        let mut provider = ServerMetricsSamplersProvider { samplers: ObjectOpenHashMap::default(), sampler_factory: ProfilerSamplerAdapter::new() };
        provider.add(tick_time);
        provider.add_all(samplers);
        provider
    }

    fn add(&mut self, sampler: MetricSampler) {
        let name = sampler.get_name().to_string();
        let hash_code = sampler.hash_code();
        self.samplers.entry_or_insert_with_hash(&name, hash_code, || sampler);
    }

    fn add_all(&mut self, samplers: Vec<MetricSampler>) {
        self.samplers.try_capacity(self.samplers.len() + samplers.len());
        for sampler in samplers {
            self.add(sampler);
        }
    }

    pub fn samplers(&mut self, profiler: &ProfilerSupplier) -> impl Iterator<Item = &mut MetricSampler> {
        let new_samplers = self.sampler_factory.new_samplers_found_in_profiler(profiler);
        self.add_all(new_samplers);
        self.samplers.iter_mut().map(|(_, sampler)| sampler)
    }

    pub fn samplers_mut(&mut self) -> impl Iterator<Item = &mut MetricSampler> {
        self.samplers.iter_mut().map(|(_, sampler)| sampler)
    }

    pub fn into_samplers(self) -> Vec<MetricSampler> {
        self.samplers.into_values()
    }
}

pub fn runtime_independent_samplers(measured: Vec<ProfilerMeasured>) -> Vec<MetricSampler> {
    let mut result = Vec::new();
    match CpuStats::new() {
        Ok(cpu_stats) => {
            let nr_of_cpus = cpu_stats.nr_of_cpus;
            let cpu_stats = Arc::new(Mutex::new(cpu_stats));
            for i in 0..nr_of_cpus {
                let cpu_stats = cpu_stats.clone();
                result.push(MetricSampler::create(&format!("cpu#{i}"), MetricCategory::Cpu, move || cpu_stats.lock().unwrap_or_else(PoisonError::into_inner).load_for_cpu(i)));
            }
        }
        Err(throwable) => warn!("Failed to query cpu, no cpu stats will be recorded\n{}", throwable.print_stack_trace().trim_end()),
    }
    result.push(MetricSampler::create("heap MiB", MetricCategory::Jvm, || f64::from(crate::system_report::size_in_mib(crate::util::java_heap::heap_memory_usage().used))));
    result.extend(get_registered_samplers(measured));
    result
}

pub fn tick_time_sampler(time_source: fn() -> i64) -> MetricSampler {
    const STOPPED: i64 = i64::MIN;
    let started_at = Arc::new(AtomicI64::new(STOPPED));
    let start = started_at.clone();
    MetricSampler::builder("ticktime", MetricCategory::TickLoop, move || {
        let started = started_at.swap(STOPPED, Ordering::Relaxed);
        if started == STOPPED { 0.0 } else { (time_source() - started) as f64 }
    })
    .with_before_tick(move || start.store(time_source(), Ordering::Relaxed))
    .with_threshold_alert(value_increased_by_percentage(2.0))
    .build()
}

pub struct CpuStats {
    processor: Box<dyn CentralProcessor>,
    pub nr_of_cpus: usize,
    previous_cpu_load_tick: Vec<[i64; TICK_TYPE_COUNT]>,
    current_load: Vec<f64>,
    last_poll_ms: i64,
}

impl CpuStats {
    pub fn new() -> OshiResult<CpuStats> {
        let system_info = SystemInfo::new();
        let processor = system_info.get_hardware().get_processor()?;
        let nr_of_cpus = processor.get_logical_processor_count().max(0) as usize;
        let previous_cpu_load_tick = processor.get_processor_cpu_load_ticks();
        let current_load = processor.get_processor_cpu_load_between_ticks(&previous_cpu_load_tick);
        Ok(CpuStats { processor, nr_of_cpus, previous_cpu_load_tick, current_load, last_poll_ms: 0 })
    }

    pub fn load_for_cpu(&mut self, i: usize) -> f64 {
        let now = crate::util::java_runtime::current_time_millis();
        if self.last_poll_ms == 0 || self.last_poll_ms + 501 < now {
            self.current_load = self.processor.get_processor_cpu_load_between_ticks(&self.previous_cpu_load_tick);
            self.previous_cpu_load_tick = self.processor.get_processor_cpu_load_ticks();
            self.last_poll_ms = now;
        }
        self.current_load[i] * 100.0
    }
}
