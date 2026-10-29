use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::crash_report::{CrashReport, ReportType, crash_report_file};
use crate::util::lifecycle::Lifecycle;
use crate::world::level::chunk::storage::chunk_io_error_reporter::ChunkIoErrorReporter;

#[derive(Debug, Default, Clone)]
pub struct ServerSnapshot {
    pub player_count: Option<String>,
    pub random_tick_rate: String,
    pub level_stats: String,
    pub world_seed: i64,
    pub world_gen_settings_lifecycle: Lifecycle,
    pub active_data_packs: String,
    pub available_data_packs: String,
    pub enabled_feature_flags: String,
    pub chunk_io_error_reporter: Option<ChunkIoErrorReporter>,
}

pub struct ServerTiming {
    epoch: Instant,
    next_tick_time_nanos: AtomicI64,
    running: AtomicBool,
    milliseconds_per_tick: AtomicU32,
    pub snapshot: Mutex<ServerSnapshot>,
}

impl ServerTiming {
    pub fn new() -> Arc<ServerTiming> {
        Arc::new(ServerTiming {
            epoch: Instant::now(),
            next_tick_time_nanos: AtomicI64::new(0),
            running: AtomicBool::new(true),
            milliseconds_per_tick: AtomicU32::new(50f32.to_bits()),
            snapshot: Mutex::new(ServerSnapshot::default()),
        })
    }

    fn nanos(&self, instant: Instant) -> i64 {
        instant.saturating_duration_since(self.epoch).as_nanos() as i64
    }

    pub fn set_next_tick_time(&self, next_tick_time: Instant, milliseconds_per_tick: f32) {
        self.next_tick_time_nanos.store(self.nanos(next_tick_time), Ordering::Relaxed);
        self.milliseconds_per_tick.store(milliseconds_per_tick.to_bits(), Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

pub fn start(timing: Arc<ServerTiming>, max_tick_time_millis: i64, server_directory: PathBuf) {
    let max_tick_time_nanos = max_tick_time_millis * 1_000_000;
    let _ = std::thread::Builder::new().name("Server Watchdog".into()).spawn(move || {
        while timing.running.load(Ordering::Relaxed) {
            let next_tick_time_nanos = timing.next_tick_time_nanos.load(Ordering::Relaxed);
            let current_time_nanos = timing.nanos(Instant::now());
            let delta_nanos = current_time_nanos - next_tick_time_nanos;
            if delta_nanos > max_tick_time_nanos {
                crash(&timing, delta_nanos, &server_directory);
            }
            let sleep = (next_tick_time_nanos + max_tick_time_nanos - current_time_nanos) / 1_000_000;
            std::thread::sleep(Duration::from_millis(sleep.max(0) as u64));
        }
    });
}

fn crash(timing: &ServerTiming, delta_nanos: i64, server_directory: &std::path::Path) -> ! {
    let milliseconds_per_tick = f32::from_bits(timing.milliseconds_per_tick.load(Ordering::Relaxed));
    error!(
        "A single server tick took {} seconds (should be max {})",
        crate::util::java_format_fixed((delta_nanos as f32 / 1.0e9) as f64, 2),
        crate::util::java_format_fixed((milliseconds_per_tick / 1000.0) as f64, 2)
    );
    error!("Considering it to be crashed, server will forcibly shutdown.");
    let snapshot = timing.snapshot.lock().map(|s| s.clone()).unwrap_or_default();
    let mut report = CrashReport::new("Watching Server", "java.lang.Error: Watchdog (Watching Server)".into(), Vec::new());
    fill_system_report(&mut report, &snapshot);
    report
        .add_category("Performance stats")
        .set_detail("Random tick rate", snapshot.random_tick_rate.clone())
        .set_detail("Level stats", snapshot.level_stats.clone());
    print!("Crash report:\n{}\n", report.get_friendly_report(&ReportType::CRASH));
    let file = crash_report_file(server_directory);
    if report.save_to_file(&file, &ReportType::CRASH) {
        error!("This crash report has been saved to: {}", std::path::absolute(&file).unwrap_or(file).display());
    } else {
        error!("We were unable to save this crash report to disk.");
    }
    exit()
}

fn exit() -> ! {
    let _ = std::thread::Builder::new().name("Timer-0".into()).spawn(|| {
        std::thread::sleep(Duration::from_millis(10000));
        crate::logging::flush();
        std::process::exit(1);
    });
    super::begin_shutdown(1);
    super::halt_and_wait();
    crate::logging::flush();
    std::process::exit(1)
}

pub fn fill_system_report(report: &mut CrashReport, snapshot: &ServerSnapshot) {
    report.set_system_detail("Server Running", "true");
    if let Some(player_count) = &snapshot.player_count {
        report.set_system_detail("Player Count", player_count.clone());
    }
    report.set_system_detail("Active Data Packs", snapshot.active_data_packs.clone());
    report.set_system_detail("Available Data Packs", snapshot.available_data_packs.clone());
    report.set_system_detail("Enabled Feature Flags", snapshot.enabled_feature_flags.clone());
    report.set_system_detail("World Generation", snapshot.world_gen_settings_lifecycle.to_string());
    report.set_system_detail("World Seed", snapshot.world_seed.to_string());
    let suppressed_exceptions = snapshot.chunk_io_error_reporter.as_ref().map_or_else(|| "~~NONE~~".to_string(), |reporter| reporter.suppressed_exceptions().dump());
    report.set_system_detail("Suppressed Exceptions", suppressed_exceptions);
    report.set_system_detail("Is Modded", "Probably not. Server jar signature and brand is untouched");
    report.set_system_detail("Type", "Dedicated Server");
}
