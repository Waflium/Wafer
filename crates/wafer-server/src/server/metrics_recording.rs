use std::sync::Arc;
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

const CONSECUTIVE_EXECUTOR_QUEUES: [&str; 7] = [
    "light-queue-size",
    "entity-deserializer-queue-size",
    "worldgen-queue-size",
    "dispatcher-queue-size",
    "IOWorker-poi-queue-size",
    "IOWorker-chunk-queue-size",
    "IOWorker-entities-queue-size",
];

fn measured(name: String, category: MetricCategory) -> ProfilerMeasured {
    ProfilerMeasured { name, category, sampler: Arc::new(|| 0.0) }
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

    pub(crate) fn end_metrics_recording_tick(&mut self) {
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

    fn profiler_measured(&self) -> Vec<ProfilerMeasured> {
        let mut measured_instances = Vec::new();
        for level in &self.levels {
            measured_instances.push(measured(format!("Chunk source main thread executor for {}-pending-tasks", level.dimension), MetricCategory::EventLoops));
            for queue in CONSECUTIVE_EXECUTOR_QUEUES {
                measured_instances.push(measured(queue.to_string(), MetricCategory::ConsecutiveExecutors));
            }
        }
        measured_instances.push(measured("Server-pending-tasks".to_string(), MetricCategory::EventLoops));
        measured_instances
    }

}
