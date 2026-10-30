use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;

use super::{MetricsReportContext, OnMetricsRecordingFinished, Server, ServerTask};
use crate::util::profiling::get_nanos;
use crate::util::profiling::metrics::active_metrics_recorder::{ActiveMetricsRecorder, EndTickOutcome};
use crate::util::profiling::metrics::metric_category::MetricCategory;
use crate::util::profiling::metrics::metrics_persister::MetricsPersister;
use crate::util::profiling::metrics::metrics_registry::ProfilerMeasured;
use crate::util::profiling::metrics::server_metrics_samplers_provider::ServerMetricsSamplersProvider;
use crate::util::profiling::profile_results::ProfileResults;
use crate::util::profiling::profiler;

type Gauge = Arc<AtomicUsize>;

#[derive(Default)]
struct LevelGauges {
    chunk_source: Gauge,
    light: Gauge,
    entity_deserializer: Gauge,
    worldgen: Gauge,
    worldgen_dispatcher: Gauge,
    light_dispatcher: Gauge,
    ticket_dispatcher: Gauge,
    poi: Gauge,
    chunk: Gauge,
    entities: Gauge,
}

#[derive(Default)]
pub(crate) struct MetricsGauges {
    levels: Vec<LevelGauges>,
    server: Gauge,
}

fn measured(name: impl Into<String>, category: MetricCategory, gauge: &Gauge) -> ProfilerMeasured {
    let gauge = gauge.clone();
    ProfilerMeasured { name: name.into(), category, sampler: Arc::new(move || gauge.load(Ordering::Relaxed) as f64) }
}

impl MetricsGauges {
    fn measured_instances(&mut self, dimensions: &[&str]) -> Vec<ProfilerMeasured> {
        self.levels = dimensions.iter().map(|_| LevelGauges::default()).collect();
        let mut measured_instances = Vec::new();
        for (dimension, gauges) in dimensions.iter().zip(&self.levels) {
            measured_instances.push(measured(format!("Chunk source main thread executor for {dimension}-pending-tasks"), MetricCategory::EventLoops, &gauges.chunk_source));
            let executors = [
                ("light", &gauges.light),
                ("entity-deserializer", &gauges.entity_deserializer),
                ("worldgen", &gauges.worldgen),
                ("dispatcher", &gauges.worldgen_dispatcher),
                ("dispatcher", &gauges.light_dispatcher),
                ("dispatcher", &gauges.ticket_dispatcher),
                ("IOWorker-poi", &gauges.poi),
                ("IOWorker-chunk", &gauges.chunk),
                ("IOWorker-entities", &gauges.entities),
            ];
            for (name, gauge) in executors {
                measured_instances.push(measured(format!("{name}-queue-size"), MetricCategory::ConsecutiveExecutors, gauge));
            }
        }
        measured_instances.push(measured("Server-pending-tasks", MetricCategory::EventLoops, &self.server));
        measured_instances
    }
}

impl Server {
    pub fn is_recording_metrics(&self) -> bool {
        self.metrics_recorder.as_ref().is_some_and(ActiveMetricsRecorder::is_recording)
    }

    pub fn start_recording_metrics(&mut self, on_stopped: impl FnOnce(&mut Server, &ProfileResults) + 'static, on_finished: OnMetricsRecordingFinished) {
        self.on_metrics_recording_stopped = Some(Box::new(move |server, report| {
            server.stop_recording_metrics();
            on_stopped(server, report);
        }));
        self.on_metrics_recording_finished = Some(on_finished);
        self.will_start_recording_metrics = true;
    }

    pub fn stop_recording_metrics(&mut self) {
        self.metrics_recorder = None;
    }

    pub fn finish_recording_metrics(&mut self) {
        if let Some(recorder) = &mut self.metrics_recorder {
            recorder.end();
        }
    }

    pub fn cancel_recording_metrics(&mut self) {
        let Some(mut recorder) = self.metrics_recorder.take() else { return };
        if recorder.cancel()
            && let Some(on_stopped) = self.on_metrics_recording_stopped.take()
        {
            on_stopped(self, &ProfileResults::Empty);
        }
    }

    pub(crate) fn create_profiler(&mut self) -> profiler::Scope {
        if self.will_start_recording_metrics {
            let provider = ServerMetricsSamplersProvider::new(get_nanos, true, self.profiler_measured());
            self.metrics_recorder = Some(ActiveMetricsRecorder::create_started(provider, get_nanos));
            self.will_start_recording_metrics = false;
        }
        match &mut self.metrics_recorder {
            Some(recorder) => {
                recorder.start_tick();
                profiler::use_fillers(recorder.get_profiler())
            }
            None => profiler::use_fillers(Vec::new()),
        }
    }

    pub(crate) fn end_metrics_recording_tick(&mut self, pending_events: usize) {
        if self.metrics_recorder.is_none() {
            return;
        }
        self.refresh_metrics_gauges(pending_events);
        let Some(recorder) = &mut self.metrics_recorder else { return };
        let EndTickOutcome::Finished(results) = recorder.end_tick() else { return };
        let recorder = self.metrics_recorder.take().expect("recorder");
        if let Some(on_stopped) = self.on_metrics_recording_stopped.take() {
            on_stopped(self, &results);
        }
        self.schedule_save_results(recorder, results);
    }

    fn schedule_save_results(&mut self, recorder: ActiveMetricsRecorder, results: ProfileResults) {
        let job = recorder.into_save_job(results);
        let on_finished = self.on_metrics_recording_finished.take();
        let Some(sender) = self.task_sender.clone() else { return };
        let spawned = std::thread::Builder::new().name(crate::server::level::chunk_workers::next_io_worker_name()).spawn(move || {
            let report_path = match job.run(&MetricsPersister::new("server")) {
                Ok(report_path) => report_path,
                Err(e) => {
                    error!("Could not save profiler results\n{e}");
                    return;
                }
            };
            let (done, finished) = channel();
            let debug_report_path = report_path.join("server");
            let io_worker = std::thread::current().name().unwrap_or_default().to_string();
            let task: ServerTask = Box::new(move |server: &mut Server| {
                server.save_debug_report(&debug_report_path, &io_worker);
                let _ = done.send(MetricsReportContext { level_name: server.level_name.clone(), snapshot: server.watchdog_snapshot() });
            });
            if sender.send(task).is_err() {
                return;
            }
            let Ok(context) = finished.recv() else { return };
            if let Some(on_finished) = on_finished {
                on_finished(report_path, context, sender);
            }
        });
        if let Err(e) = spawned {
            error!("Could not start the profiler report thread\n{e}");
        }
    }

    fn profiler_measured(&mut self) -> Vec<ProfilerMeasured> {
        let dimensions: Vec<&str> = self.levels.iter().map(|level| &*level.dimension).collect();
        self.metrics_gauges.measured_instances(&dimensions)
    }

    fn refresh_metrics_gauges(&self, pending_events: usize) {
        self.metrics_gauges.server.store(self.tasks.borrow().len() + pending_events, Ordering::Relaxed);
        for (level, gauges) in self.levels.iter().zip(&self.metrics_gauges.levels) {
            let chunks = &level.chunks;
            gauges.chunk_source.store(chunks.main_thread_pending_tasks(), Ordering::Relaxed);
            gauges.light.store(chunks.light_queue_size(), Ordering::Relaxed);
            gauges.entity_deserializer.store(level.entity_manager.loading_inbox_size(), Ordering::Relaxed);
            gauges.worldgen.store(chunks.worldgen_queue_size(), Ordering::Relaxed);
            gauges.ticket_dispatcher.store(chunks.ticket_dispatcher_queue_size(), Ordering::Relaxed);
            gauges.poi.store(chunks.poi_io_queue_size(), Ordering::Relaxed);
            gauges.chunk.store(chunks.chunk_io_queue_size(), Ordering::Relaxed);
            gauges.entities.store(level.entity_manager.io_queue_size(), Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::server::level::testing;

    #[test]
    fn every_level_registers_the_executors_vanilla_registers() {
        let mut server = testing::server();
        let measured = server.profiler_measured();
        let named = |name: &str| measured.iter().filter(|instance| instance.name == name).count();
        assert_eq!(measured.len(), 3 * 10 + 1);
        assert_eq!(named("dispatcher-queue-size"), 9);
        for name in ["light", "entity-deserializer", "worldgen", "IOWorker-poi", "IOWorker-chunk", "IOWorker-entities"] {
            assert_eq!(named(&format!("{name}-queue-size")), 3, "{name}");
        }
        assert_eq!(named("Chunk source main thread executor for minecraft:the_nether-pending-tasks"), 1);
        assert_eq!(named("Server-pending-tasks"), 1);
    }

    #[test]
    fn the_measured_values_follow_the_servers_queues() {
        let mut server = testing::server();
        let measured = server.profiler_measured();
        let value = |name: &str| (measured.iter().find(|instance| instance.name == name).unwrap().sampler)();
        assert_eq!(value("Server-pending-tasks"), 0.0);
        server.refresh_metrics_gauges(7);
        assert_eq!(value("Server-pending-tasks"), 7.0);
        assert_eq!(value("light-queue-size"), 0.0);
    }
}
