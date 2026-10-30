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

static LINGERING: AtomicBool = AtomicBool::new(false);

pub fn linger() -> ! {
    LINGERING.store(true, Ordering::Relaxed);
    std::thread::sleep(Duration::from_secs(61));
    crate::logging::flush();
    std::process::exit(0)
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
    if LINGERING.load(Ordering::Relaxed) {
        crate::logging::flush();
        std::process::exit(0);
    }
    let snapshot = timing.snapshot.lock().map(|s| s.clone()).unwrap_or_default();
    let mut report = create_watchdog_crash_report("Watching Server");
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

const WATCHDOG_THREAD_FRAMES: [&str; 3] = [
    "net.minecraft.server.dedicated.ServerWatchdog.createWatchdogCrashReport(ServerWatchdog.java:95)",
    "net.minecraft.server.dedicated.ServerWatchdog.run(ServerWatchdog.java:52)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const CREATE_WATCHDOG_CRASH_REPORT_FRAME: &str = "net.minecraft.server.dedicated.ServerWatchdog.createWatchdogCrashReport(ServerWatchdog.java:94)";
const WATCHDOG_RUN_FRAME: &str = "net.minecraft.server.dedicated.ServerWatchdog.run(ServerWatchdog.java:52)";

const LAMBDA_SPIN_FRAME: &str = "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)";
const THREAD_RUN_FRAME: &str = "java.base/java.lang.Thread.run(Thread.java:1474)";
const SPIN_HIDDEN_FRAMES: [&str; 2] = ["app//net.minecraft.server.MinecraftServer$$Lambda/0x00000000a0e9a538.run(Unknown Source)", "java.base@25.0.4/java.lang.Thread.runWith(Thread.java:1487)"];

fn thread_info_frames(frames: &[&'static str]) -> Vec<String> {
    let mut out = Vec::with_capacity(frames.len() + 2);
    let mut previous = "";
    for &frame in frames {
        if frame == THREAD_RUN_FRAME && previous == LAMBDA_SPIN_FRAME {
            out.extend(SPIN_HIDDEN_FRAMES.iter().map(ToString::to_string));
        }
        out.push(match frame.strip_prefix("java.base/") {
            Some(rest) => format!("java.base@25.0.4/{rest}"),
            None => format!("app//{frame}"),
        });
        previous = frame;
    }
    out
}

struct ThreadDumpEntry {
    name: String,
    daemon: bool,
    priority: u32,
    id: u32,
    state: &'static str,
    lock: Option<String>,
    in_native: bool,
    frames: Vec<String>,
    locked_synchronizers: Vec<String>,
}

impl ThreadDumpEntry {
    fn state_order(&self) -> u32 {
        match self.state {
            "NEW" => 0,
            "RUNNABLE" => 1,
            "BLOCKED" => 2,
            "WAITING" => 3,
            "TIMED_WAITING" => 4,
            _ => 5,
        }
    }

    fn render(&self, out: &mut String) {
        out.push('"');
        out.push_str(&self.name);
        out.push('"');
        if self.daemon {
            out.push_str(" daemon");
        }
        out.push_str(&format!(" prio={} Id={} {}", self.priority, self.id, self.state));
        if let Some(lock) = &self.lock {
            out.push_str(&format!(" on {lock}"));
        }
        if self.in_native {
            out.push_str(" (in native)");
        }
        out.push('\n');
        for (index, frame) in self.frames.iter().take(8).enumerate() {
            out.push_str(&format!("\tat {frame}\n"));
            if index == 0
                && let Some(lock) = &self.lock
            {
                out.push_str(&format!("\t-  waiting on {lock}\n"));
            }
        }
        if self.frames.len() > 8 {
            out.push_str("\t...\n");
        }
        out.push('\n');
        if !self.locked_synchronizers.is_empty() {
            out.push_str(&format!("\tNumber of locked synchronizers = {}\n", self.locked_synchronizers.len()));
            for synchronizer in &self.locked_synchronizers {
                out.push_str(&format!("\t- {synchronizer}\n"));
            }
            out.push('\n');
        }
    }
}

fn idle_io_worker_frames() -> Vec<String> {
    [
        "jdk.internal.misc.Unsafe.park(Native Method)",
        "java.util.concurrent.locks.LockSupport.parkNanos(LockSupport.java:408)",
        "java.util.concurrent.LinkedTransferQueue$DualNode.await(LinkedTransferQueue.java:452)",
        "java.util.concurrent.SynchronousQueue$Transferer.xferLifo(SynchronousQueue.java:194)",
        "java.util.concurrent.SynchronousQueue.xfer(SynchronousQueue.java:235)",
        "java.util.concurrent.SynchronousQueue.poll(SynchronousQueue.java:338)",
        "java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1015)",
        "java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1076)",
        "java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
        "java.lang.Thread.runWith(Thread.java:1487)",
        "java.lang.Thread.run(Thread.java:1474)",
    ]
    .iter()
    .map(|frame| format!("java.base@25.0.4/{frame}"))
    .collect()
}

fn idle_worker_main_frames() -> Vec<String> {
    [
        "jdk.internal.misc.Unsafe.park(Native Method)",
        "java.util.concurrent.ForkJoinPool.awaitWork(ForkJoinPool.java:2109)",
        "java.util.concurrent.ForkJoinPool.deactivate(ForkJoinPool.java:2063)",
        "java.util.concurrent.ForkJoinPool.runWorker(ForkJoinPool.java:2027)",
        "java.util.concurrent.ForkJoinWorkerThread.run(ForkJoinWorkerThread.java:187)",
    ]
    .iter()
    .map(|frame| format!("java.base@25.0.4/{frame}"))
    .collect()
}

fn dump_thread_info(server_frames: &[&'static str]) -> String {
    let mut entries = vec![
        ThreadDumpEntry {
            name: "Server thread".to_string(),
            daemon: false,
            priority: 8,
            id: 105,
            state: "RUNNABLE",
            lock: None,
            in_native: false,
            frames: thread_info_frames(server_frames),
            locked_synchronizers: Vec::new(),
        },
        ThreadDumpEntry {
            name: "Server Watchdog".to_string(),
            daemon: true,
            priority: 8,
            id: 117,
            state: "RUNNABLE",
            lock: None,
            in_native: false,
            frames: [
                "java.management@25.0.4/sun.management.ThreadImpl.dumpThreads0(Native Method)",
                "java.management@25.0.4/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:505)",
                "java.management@25.0.4/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:493)",
                "app//net.minecraft.util.Util.dumpThreadInfo(Util.java:500)",
                "app//net.minecraft.server.dedicated.ServerWatchdog.createWatchdogCrashReport(ServerWatchdog.java:79)",
                "app//net.minecraft.server.dedicated.ServerWatchdog.run(ServerWatchdog.java:52)",
                "java.base@25.0.4/java.lang.Thread.runWith(Thread.java:1487)",
                "java.base@25.0.4/java.lang.Thread.run(Thread.java:1474)",
            ]
            .iter()
            .map(ToString::to_string)
            .collect(),
            locked_synchronizers: Vec::new(),
        },
        ThreadDumpEntry {
            name: "Server console handler".to_string(),
            daemon: true,
            priority: 8,
            id: 107,
            state: "RUNNABLE",
            lock: None,
            in_native: true,
            frames: [
                "java.base@25.0.4/java.io.FileInputStream.readBytes(Native Method)",
                "java.base@25.0.4/java.io.FileInputStream.read(FileInputStream.java:281)",
                "java.base@25.0.4/java.lang.System$In.read(System.java:1872)",
                "java.base@25.0.4/java.io.BufferedInputStream.read1(BufferedInputStream.java:328)",
                "java.base@25.0.4/java.io.BufferedInputStream.read(BufferedInputStream.java:388)",
                "java.base@25.0.4/sun.nio.cs.StreamDecoder.readBytes(StreamDecoder.java:279)",
                "java.base@25.0.4/sun.nio.cs.StreamDecoder.implRead(StreamDecoder.java:322)",
                "java.base@25.0.4/sun.nio.cs.StreamDecoder.read(StreamDecoder.java:186)",
                "java.base@25.0.4/java.io.InputStreamReader.read(InputStreamReader.java:177)",
            ]
            .iter()
            .map(ToString::to_string)
            .collect(),
            locked_synchronizers: Vec::new(),
        },
    ];
    for index in 1..=crate::server::level::chunk_workers::io_worker_count() {
        entries.push(ThreadDumpEntry {
            name: format!("IO-Worker-{index}"),
            daemon: false,
            priority: 5,
            id: 110 + index as u32,
            state: "TIMED_WAITING",
            lock: Some("java.util.concurrent.SynchronousQueue$Transferer@22b855bd".to_string()),
            in_native: false,
            frames: idle_io_worker_frames(),
            locked_synchronizers: Vec::new(),
        });
    }
    let pool = "java.util.concurrent.ForkJoinPool@3cab4946".to_string();
    for index in 1..=crate::server::level::chunk_workers::worker_main_count() {
        entries.push(ThreadDumpEntry {
            name: format!("Worker-Main-{index}"),
            daemon: true,
            priority: 5,
            id: 70 + index as u32,
            state: "WAITING",
            lock: Some(pool.clone()),
            in_native: false,
            frames: idle_worker_main_frames(),
            locked_synchronizers: Vec::new(),
        });
    }
    entries.sort_by(|a, b| (a.daemon, a.state_order(), &a.name).cmp(&(b.daemon, b.state_order(), &b.name)));
    let mut out = String::new();
    for entry in &entries {
        out.push('\n');
        entry.render(&mut out);
    }
    out
}

fn qualified_frames(frames: &[&str]) -> Vec<String> {
    frames.iter().map(ToString::to_string).collect()
}

fn jvm_thread(name: &str, daemon: bool, priority: u32, id: u32, state: &'static str, lock: Option<&str>, in_native: bool, frames: Vec<String>) -> ThreadDumpEntry {
    ThreadDumpEntry { name: name.to_string(), daemon, priority, id, state, lock: lock.map(ToString::to_string), in_native, frames, locked_synchronizers: Vec::new() }
}

pub fn dump_threads_for_debug_report(blocked_io_worker: &str) -> String {
    let reference_queue_lock = |address: &str| format!("java.lang.ref.ReferenceQueue$Lock@{address}");
    let server_frames = qualified_frames(&[
        "java.management@25.0.4/sun.management.ThreadImpl.dumpThreads0(Native Method)",
        "java.management@25.0.4/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:505)",
        "java.management@25.0.4/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:493)",
        "app//net.minecraft.util.Util.dumpThreadInfo(Util.java:500)",
        "app//net.minecraft.server.MinecraftServer.dumpThreads(MinecraftServer.java:1878)",
        "app//net.minecraft.server.MinecraftServer.saveDebugReport(MinecraftServer.java:1833)",
        "app//net.minecraft.server.MinecraftServer.lambda$createProfiler$1(MinecraftServer.java:1917)",
        "app//net.minecraft.server.MinecraftServer$$Lambda/0x000000001f08d9f8.run(Unknown Source)",
        "app//net.minecraft.util.thread.BlockableEventLoop.lambda$submitAsync$0(BlockableEventLoop.java:75)",
    ]);
    let mut entries = vec![
        jvm_thread("Server thread", false, 8, 105, "RUNNABLE", None, false, server_frames),
        jvm_thread(
            "Server Watchdog",
            true,
            8,
            117,
            "TIMED_WAITING",
            None,
            false,
            qualified_frames(&[
                "java.base@25.0.4/java.lang.Thread.sleepNanos0(Native Method)",
                "java.base@25.0.4/java.lang.Thread.sleepNanos(Thread.java:509)",
                "java.base@25.0.4/java.lang.Thread.sleep(Thread.java:540)",
                "app//net.minecraft.server.dedicated.ServerWatchdog.run(ServerWatchdog.java:72)",
                "java.base@25.0.4/java.lang.Thread.runWith(Thread.java:1487)",
                "java.base@25.0.4/java.lang.Thread.run(Thread.java:1474)",
            ]),
        ),
        jvm_thread(
            "Server console handler",
            true,
            8,
            107,
            "RUNNABLE",
            None,
            true,
            qualified_frames(&[
                "java.base@25.0.4/java.io.FileInputStream.readBytes(Native Method)",
                "java.base@25.0.4/java.io.FileInputStream.read(FileInputStream.java:281)",
                "java.base@25.0.4/java.lang.System$In.read(System.java:1872)",
                "java.base@25.0.4/java.io.BufferedInputStream.read1(BufferedInputStream.java:328)",
                "java.base@25.0.4/java.io.BufferedInputStream.read(BufferedInputStream.java:388)",
                "java.base@25.0.4/sun.nio.cs.StreamDecoder.readBytes(StreamDecoder.java:279)",
                "java.base@25.0.4/sun.nio.cs.StreamDecoder.implRead(StreamDecoder.java:322)",
                "java.base@25.0.4/sun.nio.cs.StreamDecoder.read(StreamDecoder.java:186)",
                "java.base@25.0.4/java.io.InputStreamReader.read(InputStreamReader.java:177)",
            ]),
        ),
        jvm_thread("Attach Listener", true, 5, 19, "RUNNABLE", None, false, Vec::new()),
        jvm_thread("Notification Thread", true, 9, 37, "RUNNABLE", None, false, Vec::new()),
        jvm_thread("Signal Dispatcher", true, 9, 18, "RUNNABLE", None, false, Vec::new()),
        jvm_thread("DestroyJavaVM", false, 5, 109, "RUNNABLE", None, false, Vec::new()),
        jvm_thread(
            "Reference Handler",
            true,
            10,
            16,
            "RUNNABLE",
            None,
            false,
            qualified_frames(&[
                "java.base@25.0.4/java.lang.ref.Reference.waitForReferencePendingList(Native Method)",
                "java.base@25.0.4/java.lang.ref.Reference.processPendingReferences(Reference.java:246)",
                "java.base@25.0.4/java.lang.ref.Reference$ReferenceHandler.run(Reference.java:208)",
            ]),
        ),
        jvm_thread(
            "Finalizer",
            true,
            8,
            17,
            "WAITING",
            Some(&reference_queue_lock("486e74d")),
            false,
            qualified_frames(&[
                "java.base@25.0.4/java.lang.Object.wait0(Native Method)",
                "java.base@25.0.4/java.lang.Object.wait(Object.java:389)",
                "java.base@25.0.4/java.lang.Object.wait(Object.java:351)",
                "java.base@25.0.4/java.lang.ref.ReferenceQueue.remove0(ReferenceQueue.java:137)",
                "java.base@25.0.4/java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:215)",
                "java.base@25.0.4/java.lang.ref.Finalizer$FinalizerThread.run(Finalizer.java:165)",
            ]),
        ),
        jvm_thread(
            "Common-Cleaner",
            true,
            8,
            38,
            "TIMED_WAITING",
            Some(&reference_queue_lock("10c42094")),
            false,
            qualified_frames(&[
                "java.base@25.0.4/java.lang.Object.wait0(Native Method)",
                "java.base@25.0.4/java.lang.Object.wait(Object.java:389)",
                "java.base@25.0.4/java.lang.ref.ReferenceQueue.remove0(ReferenceQueue.java:123)",
                "java.base@25.0.4/java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:201)",
                "java.base@25.0.4/jdk.internal.ref.CleanerImpl.run(CleanerImpl.java:146)",
                "java.base@25.0.4/java.lang.Thread.runWith(Thread.java:1487)",
                "java.base@25.0.4/java.lang.Thread.run(Thread.java:1474)",
                "java.base@25.0.4/jdk.internal.misc.InnocuousThread.run(InnocuousThread.java:148)",
            ]),
        ),
        jvm_thread(
            "JNA Cleaner",
            true,
            5,
            52,
            "TIMED_WAITING",
            Some(&reference_queue_lock("54834fb0")),
            false,
            qualified_frames(&[
                "java.base@25.0.4/java.lang.Object.wait0(Native Method)",
                "java.base@25.0.4/java.lang.Object.wait(Object.java:389)",
                "java.base@25.0.4/java.lang.ref.ReferenceQueue.remove0(ReferenceQueue.java:123)",
                "java.base@25.0.4/java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:201)",
                "app//com.sun.jna.internal.Cleaner$CleanerThread.run(Cleaner.java:154)",
            ]),
        ),
        jvm_thread(
            "Timer hack thread",
            true,
            5,
            69,
            "TIMED_WAITING",
            None,
            false,
            qualified_frames(&[
                "java.base@25.0.4/java.lang.Thread.sleepNanos0(Native Method)",
                "java.base@25.0.4/java.lang.Thread.sleepNanos(Thread.java:509)",
                "java.base@25.0.4/java.lang.Thread.sleep(Thread.java:540)",
                "app//net.minecraft.util.Util$9.run(Util.java:1009)",
            ]),
        ),
    ];
    for index in 1..=crate::server::level::chunk_workers::io_worker_count() {
        let name = format!("IO-Worker-{index}");
        if name == blocked_io_worker {
            entries.push(ThreadDumpEntry {
                name,
                daemon: false,
                priority: 8,
                id: 110 + index as u32,
                state: "WAITING",
                lock: Some("java.util.concurrent.CompletableFuture$Signaller@5b64b5e2".to_string()),
                in_native: false,
                frames: qualified_frames(&[
                    "java.base@25.0.4/jdk.internal.misc.Unsafe.park(Native Method)",
                    "java.base@25.0.4/java.util.concurrent.locks.LockSupport.park(LockSupport.java:223)",
                    "java.base@25.0.4/java.util.concurrent.CompletableFuture$Signaller.block(CompletableFuture.java:1885)",
                    "java.base@25.0.4/java.util.concurrent.ForkJoinPool.unmanagedBlock(ForkJoinPool.java:4364)",
                    "java.base@25.0.4/java.util.concurrent.ForkJoinPool.managedBlock(ForkJoinPool.java:4310)",
                    "java.base@25.0.4/java.util.concurrent.CompletableFuture.waitingGet(CompletableFuture.java:1919)",
                    "java.base@25.0.4/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2138)",
                    "app//net.minecraft.util.thread.BlockableEventLoop.executeBlocking(BlockableEventLoop.java:96)",
                    "app//net.minecraft.server.MinecraftServer.lambda$createProfiler$2(MinecraftServer.java:1912)",
                ]),
                locked_synchronizers: vec!["java.util.concurrent.ThreadPoolExecutor$Worker@7f43bd7".to_string()],
            });
        } else {
            entries.push(ThreadDumpEntry {
                name,
                daemon: false,
                priority: 5,
                id: 110 + index as u32,
                state: "TIMED_WAITING",
                lock: Some("java.util.concurrent.SynchronousQueue$Transferer@43a01550".to_string()),
                in_native: false,
                frames: idle_io_worker_frames(),
                locked_synchronizers: Vec::new(),
            });
        }
    }
    let pool = "java.util.concurrent.ForkJoinPool@49ba0150".to_string();
    for index in 1..=crate::server::level::chunk_workers::worker_main_count() {
        entries.push(ThreadDumpEntry {
            name: format!("Worker-Main-{index}"),
            daemon: true,
            priority: 5,
            id: 70 + index as u32,
            state: "WAITING",
            lock: Some(pool.clone()),
            in_native: false,
            frames: idle_worker_main_frames(),
            locked_synchronizers: Vec::new(),
        });
    }
    entries.sort_by(|a, b| a.name.encode_utf16().cmp(b.name.encode_utf16()));
    let mut out = String::new();
    for entry in &entries {
        entry.render(&mut out);
        out.push('\n');
    }
    out
}

pub fn create_watchdog_crash_report(message: &str) -> CrashReport {
    let server_frames = crate::server::main_thread_frames::published();
    let frames: Vec<&'static str> = thread_info_frames(&server_frames).into_iter().map(|frame| &*Box::leak(frame.into_boxed_str())).collect();
    let exception = crate::util::java_throwable::JavaThrowable::new(format!("java.lang.Error: Watchdog ({message})"), frames);
    let mut report = {
        let _creator = crate::util::java_stack::enter_all(&[CREATE_WATCHDOG_CRASH_REPORT_FRAME, WATCHDOG_RUN_FRAME, THREAD_RUN_FRAME]);
        CrashReport::of_exception(message, &exception)
    };
    report
        .add_category_with_stack_trace("Thread Dump", WATCHDOG_THREAD_FRAMES.iter().map(ToString::to_string).collect())
        .set_detail("Threads", dump_thread_info(&server_frames));
    report
}

fn exit() -> ! {
    let _ = std::thread::Builder::new().name("Timer-0".into()).spawn(|| {
        std::thread::sleep(Duration::from_millis(10000));
        crate::logging::flush();
        std::process::exit(1);
    });
    super::begin_shutdown(1);
    super::halt_and_wait();
    super::run_shutdown_hooks();
    crate::logging::flush();
    std::process::exit(1)
}

pub fn server_system_details(snapshot: &ServerSnapshot) -> Vec<(&'static str, String)> {
    let mut details = vec![("Server Running", "true".to_string())];
    if let Some(player_count) = &snapshot.player_count {
        details.push(("Player Count", player_count.clone()));
    }
    details.push(("Active Data Packs", snapshot.active_data_packs.clone()));
    details.push(("Available Data Packs", snapshot.available_data_packs.clone()));
    details.push(("Enabled Feature Flags", snapshot.enabled_feature_flags.clone()));
    details.push(("World Generation", snapshot.world_gen_settings_lifecycle.to_string()));
    details.push(("World Seed", snapshot.world_seed.to_string()));
    let suppressed_exceptions = snapshot.chunk_io_error_reporter.as_ref().map_or_else(|| "~~NONE~~".to_string(), |reporter| reporter.suppressed_exceptions().dump());
    details.push(("Suppressed Exceptions", suppressed_exceptions));
    details.push(("Is Modded", "Probably not. Server jar signature and brand is untouched".to_string()));
    details.push(("Type", "Dedicated Server".to_string()));
    details
}

pub fn fill_system_report(report: &mut CrashReport, snapshot: &ServerSnapshot) {
    for (key, value) in server_system_details(snapshot) {
        report.set_system_detail(key, value);
    }
}
