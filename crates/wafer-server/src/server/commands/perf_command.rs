use std::path::{Path, PathBuf};

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::logging::with_thread_name;
use crate::network::chat::{Arg, Component};
use crate::server::{MetricsReportContext, Server, ServerTaskSender};
use crate::system_report::SystemReport;
use crate::util::file_zipper::FileZipper;
use crate::util::java_format_fixed;
use crate::util::profiling::metrics::metrics_persister::PROFILING_RESULTS_DIR;
use crate::util::profiling::profile_results::ProfileResults;

static ERROR_NOT_RUNNING: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.perf.notRunning");
static ERROR_ALREADY_RUNNING: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.perf.alreadyRunning");

const SAVE_RESULTS_SYSTEM_REPORT_FRAMES: [&str; 7] = [
    "net.minecraft.server.commands.PerfCommand.saveResults(PerfCommand.java:85)",
    "net.minecraft.server.commands.PerfCommand.lambda$startProfilingDedicatedServer$1(PerfCommand.java:52)",
    "net.minecraft.server.MinecraftServer.lambda$createProfiler$0(MinecraftServer.java:1918)",
    "net.minecraft.util.profiling.metrics.profiling.ActiveMetricsRecorder.lambda$scheduleSaveResults$0(ActiveMetricsRecorder.java:159)",
    "java.base/java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1090)",
    "java.base/java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

pub fn start(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if server.is_recording_metrics() {
        return Err(ERROR_ALREADY_RUNNING.create());
    }
    let stopped_source = source.clone();
    let finished_source = source.clone();
    server.start_recording_metrics(
        move |server, results| when_stopped(server, &stopped_source, results),
        Box::new(move |report, context, sender| save_results(finished_source, report, context, sender)),
    );
    source.send_success(server, || Component::translatable("commands.perf.started"), false);
    Ok(0)
}

pub fn stop(server: &mut Server, _context: &CommandContext, _source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.is_recording_metrics() {
        return Err(ERROR_NOT_RUNNING.create());
    }
    server.finish_recording_metrics();
    Ok(0)
}

fn on_server_thread(sender: &ServerTaskSender, thread_name: &'static str, task: impl FnOnce(&mut Server) + Send + Sync + 'static) {
    let _ = sender.send(Box::new(move |server: &mut Server| with_thread_name(thread_name, || task(server))));
}

fn save_results(source: CommandSourceStack, report: PathBuf, context: MetricsReportContext, sender: ServerTaskSender) {
    let thread_name: &'static str = Box::leak(std::thread::current().name().unwrap_or_default().to_string().into_boxed_str());
    let profiling_name = format!("{}-{}-{}", crate::util::get_filename_formatted_date_time(), context.level_name, wafer_data::VERSION_NAME);
    let results_dir = Path::new(PROFILING_RESULTS_DIR);
    let zip_file = match crate::util::file_util::find_available_name(results_dir, &profiling_name, ".zip") {
        Ok(zip_file) => zip_file,
        Err(e) => {
            on_server_thread(&sender, thread_name, move |server| source.send_failure(server, Component::translatable("commands.perf.reportFailed")));
            error!("Failed to create report name\n{e}");
            return;
        }
    };
    if let Err(e) = write_zip(&results_dir.join(&zip_file), &report, &context) {
        error!("Failed to write the profiling report\n{e}");
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(&report) {
        warn!("Failed to delete temporary profiling file {}\n{e}", crate::util::java_nio::path_string(&report));
    }
    on_server_thread(&sender, thread_name, move |server| {
        source.send_success(server, || tr("commands.perf.reportSaved", vec![Arg::String(zip_file)]), false);
    });
}

fn write_zip(zip_path: &Path, report: &Path, context: &MetricsReportContext) -> std::io::Result<()> {
    let mut file_zipper = FileZipper::new(zip_path)?;
    let mut system_report = {
        let _caller = crate::util::java_stack::enter_all(&SAVE_RESULTS_SYSTEM_REPORT_FRAMES);
        SystemReport::new()
    };
    for (key, value) in crate::server::dedicated::server_watchdog::server_system_details(&context.snapshot) {
        system_report.set_detail_value(key, value);
    }
    file_zipper.add_string("system.txt", &system_report.to_line_separated_string())?;
    file_zipper.add_directory_contents(report)?;
    file_zipper.close()
}

fn when_stopped(server: &Server, source: &CommandSourceStack, results: &ProfileResults) {
    if !results.is_empty() {
        let ticks = results.get_tick_duration();
        let duration_in_seconds = results.get_nano_duration() as f64 / 1.0e9;
        source.send_success(
            server,
            || {
                tr(
                    "commands.perf.stopped",
                    vec![Arg::String(java_format_fixed(duration_in_seconds, 2)), Arg::Int(ticks), Arg::String(java_format_fixed(f64::from(ticks) / duration_in_seconds, 2))],
                )
            },
            false,
        );
    }
}
