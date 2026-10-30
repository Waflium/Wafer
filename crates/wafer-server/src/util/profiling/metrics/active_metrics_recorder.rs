use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use super::metric_sampler::{MetricSampler, SamplingPhase};
use super::metrics_persister::{MetricsPersister, RecordedDeviation};
use super::server_metrics_samplers_provider::{ProfilerSupplier, ServerMetricsSamplersProvider};
use crate::util::fastutil::java_hash_set_copy_order;
use crate::util::profiling::active_profiler::{ActiveProfiler, SharedProfiler, lock};
use crate::util::profiling::continuous_profiler::ContinuousProfiler;
use crate::util::profiling::profile_results::ProfileResults;

pub const PROFILING_MAX_DURATION_SECONDS: i64 = 10;

type SingleTickProfiler = Arc<Mutex<Option<SharedProfiler>>>;

pub enum EndTickOutcome {
    Continue,
    Finished(ProfileResults),
}

pub struct ActiveMetricsRecorder {
    deviations_by_sampler: Vec<(String, Vec<RecordedDeviation>)>,
    task_profiler: ContinuousProfiler,
    metrics_sampler_provider: ServerMetricsSamplersProvider,
    wall_time_source: fn() -> i64,
    deadline_nano: i64,
    current_tick: Arc<AtomicI32>,
    single_tick_profiler: SingleTickProfiler,
    kill_switch: bool,
}

pub struct SaveJob {
    samplers: Vec<MetricSampler>,
    deviations_by_sampler: Vec<(String, Vec<RecordedDeviation>)>,
    results: ProfileResults,
}

impl ActiveMetricsRecorder {
    pub fn create_started(metrics_sampler_provider: ServerMetricsSamplersProvider, time_source: fn() -> i64) -> ActiveMetricsRecorder {
        let current_tick = Arc::new(AtomicI32::new(0));
        let mut task_profiler = ContinuousProfiler::new(current_tick.clone(), false);
        let single_tick_profiler: SingleTickProfiler = Arc::new(Mutex::new(Some(new_single_tick_profiler(&current_tick))));
        task_profiler.enable();
        ActiveMetricsRecorder {
            deviations_by_sampler: Vec::new(),
            task_profiler,
            metrics_sampler_provider,
            wall_time_source: time_source,
            deadline_nano: time_source() + PROFILING_MAX_DURATION_SECONDS * 1_000_000_000,
            current_tick,
            single_tick_profiler,
            kill_switch: false,
        }
    }

    pub fn end(&mut self) {
        if self.is_recording() {
            self.kill_switch = true;
        }
    }

    pub fn cancel(&mut self) -> bool {
        if !self.is_recording() {
            return false;
        }
        *self.single_tick_profiler.lock().unwrap_or_else(PoisonError::into_inner) = None;
        self.cleanup();
        true
    }

    pub fn start_tick(&mut self) {
        self.verify_started();
        let cell = self.single_tick_profiler.clone();
        let supplier: ProfilerSupplier = Arc::new(move || cell.lock().unwrap_or_else(PoisonError::into_inner).clone());
        for sampler in self.metrics_sampler_provider.samplers(&supplier) {
            sampler.on_start_tick();
        }
        self.current_tick.fetch_add(1, Ordering::Relaxed);
    }

    pub fn sample_during_extract(&mut self) {
        self.sample(SamplingPhase::Extract);
    }

    pub fn end_tick(&mut self) -> EndTickOutcome {
        self.sample(SamplingPhase::EndTick);
        if !self.kill_switch && (self.wall_time_source)() <= self.deadline_nano {
            *self.single_tick_profiler.lock().unwrap_or_else(PoisonError::into_inner) = Some(new_single_tick_profiler(&self.current_tick));
            EndTickOutcome::Continue
        } else {
            self.kill_switch = false;
            let results = self.task_profiler.get_results();
            *self.single_tick_profiler.lock().unwrap_or_else(PoisonError::into_inner) = None;
            EndTickOutcome::Finished(results)
        }
    }

    fn sample(&mut self, sampling_phase: SamplingPhase) {
        self.verify_started();
        let current_tick = self.current_tick.load(Ordering::Relaxed);
        if current_tick == 0 {
            return;
        }
        let single_tick_profiler = self.single_tick_profiler.lock().unwrap_or_else(PoisonError::into_inner).clone();
        for sampler in self.metrics_sampler_provider.samplers_mut() {
            if sampler.sampling_phase() == sampling_phase {
                sampler.on_end_tick(current_tick);
                if sampler.triggers_threshold() {
                    let results = single_tick_profiler.as_ref().map_or(ProfileResults::Empty, |profiler| lock(profiler).get_results());
                    let recorded_deviation = RecordedDeviation { timestamp: SystemTime::now(), tick: current_tick, profiler_result_at_tick: results };
                    match self.deviations_by_sampler.iter_mut().find(|(name, _)| name == sampler.get_name()) {
                        Some((_, deviations)) => deviations.push(recorded_deviation),
                        None => self.deviations_by_sampler.push((sampler.get_name().to_string(), vec![recorded_deviation])),
                    }
                }
            }
        }
    }

    pub fn is_recording(&self) -> bool {
        self.task_profiler.is_enabled()
    }

    pub fn get_profiler(&self) -> Vec<SharedProfiler> {
        let mut profilers: Vec<SharedProfiler> = self.task_profiler.get_filler().into_iter().collect();
        profilers.extend(self.single_tick_profiler.lock().unwrap_or_else(PoisonError::into_inner).clone());
        profilers
    }

    fn verify_started(&self) {
        assert!(self.is_recording(), "Not started!");
    }

    fn cleanup(&mut self) {
        for sampler in self.metrics_sampler_provider.samplers_mut() {
            sampler.on_finished();
        }
        self.deviations_by_sampler.clear();
        self.task_profiler.disable();
    }

    pub fn into_save_job(self, results: ProfileResults) -> SaveJob {
        let ActiveMetricsRecorder { deviations_by_sampler, metrics_sampler_provider, .. } = self;
        let samplers = metrics_sampler_provider.into_samplers();
        let hashes: Vec<i32> = samplers.iter().map(MetricSampler::hash_code).collect();
        let mut samplers: Vec<Option<MetricSampler>> = samplers.into_iter().map(Some).collect();
        let samplers = java_hash_set_copy_order(&hashes).into_iter().filter_map(|index| samplers[index].take()).collect();
        SaveJob { samplers, deviations_by_sampler, results }
    }
}

fn new_single_tick_profiler(current_tick: &Arc<AtomicI32>) -> SharedProfiler {
    let current_tick = current_tick.clone();
    Arc::new(Mutex::new(ActiveProfiler::new(Box::new(move || current_tick.load(Ordering::Relaxed)), true)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::profiling::get_nanos;
    use crate::util::profiling::metrics::metric_category::MetricCategory;
    use crate::util::profiling::metrics::metrics_registry::{ProfilerMeasured, get_registered_samplers};
    use crate::util::profiling::metrics::server_metrics_samplers_provider::tick_time_sampler;

    fn measured(name: &str, category: MetricCategory) -> ProfilerMeasured {
        ProfilerMeasured { name: name.to_string(), category, sampler: Arc::new(|| 0.0) }
    }

    fn columns(job: &SaveJob, category: MetricCategory) -> Vec<String> {
        job.samplers.iter().filter(|sampler| sampler.get_category() == category).map(|sampler| sampler.get_name().to_string()).collect()
    }

    #[test]
    fn csv_columns_follow_java_hash_set_order_like_vanilla() {
        let mut samplers = Vec::new();
        for i in 0..32 {
            samplers.push(MetricSampler::create(&format!("cpu#{i}"), MetricCategory::Cpu, || 0.0));
        }
        samplers.push(MetricSampler::create("heap MiB", MetricCategory::Jvm, || 0.0));
        let mut measured_instances = Vec::new();
        for dimension in ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"] {
            measured_instances.push(measured(&format!("Chunk source main thread executor for {dimension}-pending-tasks"), MetricCategory::EventLoops));
            for queue in ["light", "entity-deserializer", "worldgen", "dispatcher", "dispatcher", "dispatcher", "IOWorker-poi", "IOWorker-chunk", "IOWorker-entities"] {
                measured_instances.push(measured(&format!("{queue}-queue-size"), MetricCategory::ConsecutiveExecutors));
            }
        }
        measured_instances.push(measured("Server-pending-tasks", MetricCategory::EventLoops));
        samplers.extend(get_registered_samplers(measured_instances));
        let recorder = ActiveMetricsRecorder::create_started(ServerMetricsSamplersProvider::with_samplers(tick_time_sampler(get_nanos), samplers), get_nanos);
        let job = recorder.into_save_job(ProfileResults::Empty);
        let cpus: Vec<String> = [29, 28, 27, 15, 14, 13, 12, 11, 10, 31, 30, 19, 18, 17, 16, 26, 2, 1, 25, 0, 24, 23, 22, 21, 20, 9, 8, 7, 6, 5, 4, 3].iter().map(|i| format!("cpu#{i}")).collect();
        assert_eq!(columns(&job, MetricCategory::Cpu), cpus);
        let expected = ["light", "entity-deserializer", "worldgen", "dispatcher", "IOWorker-poi", "IOWorker-chunk", "IOWorker-entities"].map(|queue| format!("{queue}-queue-size"));
        assert_eq!(columns(&job, MetricCategory::ConsecutiveExecutors), expected);
        let event_loops = [
            "Chunk source main thread executor for minecraft:the_end-pending-tasks",
            "Chunk source main thread executor for minecraft:overworld-pending-tasks",
            "Chunk source main thread executor for minecraft:the_nether-pending-tasks",
            "Server-pending-tasks",
        ];
        assert_eq!(columns(&job, MetricCategory::EventLoops), event_loops);
    }
}

impl SaveJob {
    pub fn run(mut self, metrics_persister: &MetricsPersister) -> std::io::Result<PathBuf> {
        let path_to_logs = metrics_persister.save_reports(&self.samplers, &self.deviations_by_sampler, &self.results);
        for sampler in &mut self.samplers {
            sampler.on_finished();
        }
        path_to_logs
    }
}
