use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::crash_report::{CrashReport, ReportType, crash_report_file};
use crate::server::level::chunk_level::ChunkStatus;
use crate::server::rcon::GenericThreadKind;
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

static WATCHDOG_ID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

fn watchdog_id() -> u32 {
    *WATCHDOG_ID.get_or_init(crate::util::next_java_thread_id)
}

pub fn start(timing: Arc<ServerTiming>, max_tick_time_millis: i64, server_directory: PathBuf) {
    let max_tick_time_nanos = max_tick_time_millis * 1_000_000;
    watchdog_id();
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
const SPIN_HIDDEN_FRAMES: [&str; 2] = ["net.minecraft.server.MinecraftServer$$Lambda/0x00000000a0e9a538.run(Unknown Source)", "java.base/java.lang.Thread.runWith(Thread.java:1487)"];

fn runtime_frame(frame: &str) -> String {
    match frame.split_once('/') {
        Some((module, rest)) if module == "java.base" || module == "java.management" => format!("{module}@{}/{rest}", crate::util::java_runtime::java_version()),
        _ => format!("app//{frame}"),
    }
}

fn thread_info_frames(frames: &[&'static str]) -> Vec<String> {
    let mut out = Vec::with_capacity(frames.len() + 2);
    let mut previous = "";
    for &frame in frames {
        if frame == THREAD_RUN_FRAME && previous == LAMBDA_SPIN_FRAME {
            out.extend(SPIN_HIDDEN_FRAMES.iter().map(ToString::to_string));
        }
        if let Some(proxy) = crate::server::command_frames::lambda_proxy_frame(previous) {
            out.push(proxy.to_string());
        }
        out.push(frame.to_string());
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
    locked_monitors: Vec<(usize, String)>,
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
            out.push_str(&format!("\tat {}\n", runtime_frame(frame)));
            if index == 0
                && let Some(lock) = &self.lock
            {
                out.push_str(&format!("\t-  waiting on {lock}\n"));
            }
            for (_, monitor) in self.locked_monitors.iter().filter(|(depth, _)| *depth == index) {
                out.push_str(&format!("\t-  locked {monitor}\n"));
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
    .map(|frame| format!("java.base/{frame}"))
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
    .map(|frame| format!("java.base/{frame}"))
    .collect()
}

const SIGNALLER_JOIN_FRAMES: [&str; 7] = [
    "java.base/jdk.internal.misc.Unsafe.park(Native Method)",
    "java.base/java.util.concurrent.locks.LockSupport.park(LockSupport.java:223)",
    "java.base/java.util.concurrent.CompletableFuture$Signaller.block(CompletableFuture.java:1885)",
    "java.base/java.util.concurrent.ForkJoinPool.unmanagedBlock(ForkJoinPool.java:4364)",
    "java.base/java.util.concurrent.ForkJoinPool.managedBlock(ForkJoinPool.java:4310)",
    "java.base/java.util.concurrent.CompletableFuture.waitingGet(CompletableFuture.java:1919)",
    "java.base/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2138)",
];

fn services_discovery_thread() -> ThreadDumpEntry {
    let frames = SIGNALLER_JOIN_FRAMES
        .iter()
        .chain(
            [
                "com.mojang.authlib.services.RetryableFetch.lambda$fetch$0(RetryableFetch.java:69)",
                "com.mojang.authlib.services.RetryableFetch$$Lambda/0x0000000094c13660.get(Unknown Source)",
                "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getEndpoint(MinecraftServicesDiscoveryService.java:136)",
            ]
            .iter(),
        )
        .map(|frame| thread_info_frames(&[*frame]).remove(0))
        .collect();
    ThreadDumpEntry {
        name: "Minecraft Services Discovery".to_string(),
        daemon: true,
        priority: 5,
        id: 65,
        state: "WAITING",
        lock: Some("java.util.concurrent.CompletableFuture$Signaller@761f3cee".to_string()),
        in_native: false,
        frames,
        locked_monitors: Vec::new(),
        locked_synchronizers: vec!["java.util.concurrent.ThreadPoolExecutor$Worker@72eae541".to_string()],
    }
}

fn services_discovery_idle_thread() -> ThreadDumpEntry {
    let frames = [
        "java.base/jdk.internal.misc.Unsafe.park(Native Method)",
        "java.base/java.util.concurrent.locks.LockSupport.parkNanos(LockSupport.java:271)",
        "java.base/java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.awaitNanos(AbstractQueuedSynchronizer.java:1803)",
        "java.base/java.util.concurrent.ScheduledThreadPoolExecutor$DelayedWorkQueue.take(ScheduledThreadPoolExecutor.java:1166)",
        "java.base/java.util.concurrent.ScheduledThreadPoolExecutor$DelayedWorkQueue.take(ScheduledThreadPoolExecutor.java:883)",
        "java.base/java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1016)",
        "java.base/java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1076)",
        "java.base/java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
        "java.base/java.lang.Thread.runWith(Thread.java:1487)",
        "java.base/java.lang.Thread.run(Thread.java:1474)",
    ]
    .iter()
    .map(|frame| thread_info_frames(&[*frame]).remove(0))
    .collect();
    ThreadDumpEntry {
        name: "Minecraft Services Discovery".to_string(),
        daemon: true,
        priority: 5,
        id: 65,
        state: "TIMED_WAITING",
        lock: Some("java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject@1c22eb9f".to_string()),
        in_native: false,
        frames,
        locked_monitors: Vec::new(),
        locked_synchronizers: Vec::new(),
    }
}

const SERVER_THREAD_ID: u32 = 100;

fn worker_main_id(index: usize) -> u32 {
    if index <= 10 { 65 + index as u32 } else { 66 + index as u32 }
}

fn console_handler_thread() -> ThreadDumpEntry {
    ThreadDumpEntry {
        name: "Server console handler".to_string(),
        daemon: true,
        priority: 8,
        id: 103,
        state: "RUNNABLE",
        lock: None,
        in_native: true,
        frames: qualified_frames(&[
            "java.base/java.io.FileInputStream.readBytes(Native Method)",
            "java.base/java.io.FileInputStream.read(FileInputStream.java:281)",
            "java.base/java.lang.System$In.read(System.java:1871)",
            "java.base/java.io.BufferedInputStream.read1(BufferedInputStream.java:328)",
            "java.base/java.io.BufferedInputStream.read(BufferedInputStream.java:388)",
            "java.base/sun.nio.cs.StreamDecoder.readBytes(StreamDecoder.java:279)",
            "java.base/sun.nio.cs.StreamDecoder.implRead(StreamDecoder.java:322)",
            "java.base/sun.nio.cs.StreamDecoder.read(StreamDecoder.java:186)",
            "java.base/java.io.InputStreamReader.read(InputStreamReader.java:177)",
        ]),
        locked_monitors: vec![(4, "java.io.BufferedInputStream@ab79b36".to_string()), (7, "java.io.InputStreamReader@7c3a3cd7".to_string())],
        locked_synchronizers: Vec::new(),
    }
}

fn netty_io_threads() -> Vec<ThreadDumpEntry> {
    std::iter::once(105).chain(crate::network::server_connection_listener::child_event_loop_ids()).enumerate().map(|(index, id)| netty_io_thread(index, id)).collect()
}

fn netty_io_thread(index: usize, id: u32) -> ThreadDumpEntry {
    ThreadDumpEntry {
        name: format!("Netty NIO IO #{index}"),
        daemon: true,
        priority: 5,
        id,
        state: "RUNNABLE",
        lock: None,
        in_native: true,
        frames: qualified_frames(&[
            "java.base/sun.nio.ch.WEPoll.wait(Native Method)",
            "java.base/sun.nio.ch.WEPollSelectorImpl.doSelect(WEPollSelectorImpl.java:108)",
            "java.base/sun.nio.ch.SelectorImpl.lockAndDoSelect(SelectorImpl.java:130)",
            "java.base/sun.nio.ch.SelectorImpl.select(SelectorImpl.java:142)",
            "io.netty.channel.nio.NioIoHandler.select(NioIoHandler.java:668)",
            "io.netty.channel.nio.NioIoHandler.run(NioIoHandler.java:434)",
            "io.netty.channel.SingleThreadIoEventLoop.runIo(SingleThreadIoEventLoop.java:225)",
            "io.netty.channel.SingleThreadIoEventLoop.run(SingleThreadIoEventLoop.java:196)",
            "io.netty.util.concurrent.SingleThreadEventExecutor$5.run(SingleThreadEventExecutor.java:1195)",
        ]),
        locked_monitors: vec![(2, "sun.nio.ch.Util$2@6888fd21".to_string()), (2, "sun.nio.ch.WEPollSelectorImpl@5ed0b52d".to_string())],
        locked_synchronizers: vec!["java.util.concurrent.locks.ReentrantLock$NonfairSync@53efc31a".to_string()],
    }
}

fn worker_main_threads(pool: &str) -> Vec<ThreadDumpEntry> {
    let running = crate::server::level::chunk_workers::running_worker_steps();
    (1..=crate::server::level::chunk_workers::worker_main_count())
        .map(|index| {
            let step = running.iter().find(|(worker, _)| *worker == index).map(|(_, status)| *status);
            ThreadDumpEntry {
                name: format!("Worker-Main-{index}"),
                daemon: true,
                priority: 5,
                id: worker_main_id(index),
                state: if step.is_some() { "RUNNABLE" } else { "WAITING" },
                lock: step.is_none().then(|| pool.to_string()),
                in_native: false,
                frames: step.map_or_else(idle_worker_main_frames, |status| qualified_frames(running_worker_main_frames(status))),
                locked_monitors: Vec::new(),
                locked_synchronizers: Vec::new(),
            }
        })
        .collect()
}

fn running_worker_main_frames(status: ChunkStatus) -> &'static [&'static str] {
    match status {
        ChunkStatus::Empty => &WORKER_MAIN_LOAD_FRAMES,
        ChunkStatus::StructureStarts => &WORKER_MAIN_STRUCTURE_STARTS_FRAMES,
        _ => &WORKER_MAIN_TERRAIN_FRAMES,
    }
}

const WORKER_MAIN_STRUCTURE_STARTS_FRAMES: [&str; 9] = [
    "net.minecraft.world.level.levelgen.BitRandomSource.nextInt(BitRandomSource.java:25)",
    "net.minecraft.util.Util.shuffle(Util.java:1100)",
    "net.minecraft.world.level.levelgen.structure.pools.SinglePoolElement.getShuffledJigsawBlocks(SinglePoolElement.java:121)",
    "net.minecraft.world.level.levelgen.structure.pools.JigsawPlacement$Placer.tryPlacingChildren(JigsawPlacement.java:144)",
    "net.minecraft.world.level.levelgen.structure.pools.JigsawPlacement.addPieces(JigsawPlacement.java:379)",
    "net.minecraft.world.level.levelgen.structure.pools.JigsawPlacement.lambda$addPieces$2(JigsawPlacement.java:343)",
    "net.minecraft.world.level.levelgen.structure.pools.JigsawPlacement$$Lambda/0x0000000082f273c8.accept(Unknown Source)",
    "net.minecraft.world.level.levelgen.structure.Structure$GenerationStub.lambda$getPiecesBuilder$0(Structure.java:197)",
    "net.minecraft.world.level.levelgen.structure.Structure$GenerationStub$$Lambda/0x0000000082f27868.apply(Unknown Source)",
];

const WORKER_MAIN_LOAD_FRAMES: [&str; 9] = [
    "java.base/java.util.HashMap.getNode(HashMap.java:579)",
    "java.base/java.util.HashMap.get(HashMap.java:565)",
    "net.minecraft.nbt.CompoundTag.getCompound(CompoundTag.java:421)",
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(SerializableChunkData.java:187)",
    "net.minecraft.server.level.ChunkMap.lambda$scheduleChunkLoad$1(ChunkMap.java:585)",
    "net.minecraft.server.level.ChunkMap$$Lambda/0x00000000abf47220.apply(Unknown Source)",
    "java.base/java.util.Optional.map(Optional.java:260)",
    "net.minecraft.server.level.ChunkMap.lambda$scheduleChunkLoad$0(ChunkMap.java:584)",
    "net.minecraft.server.level.ChunkMap$$Lambda/0x00000000abf44b48.apply(Unknown Source)",
];

const WORKER_MAIN_TERRAIN_FRAMES: [&str; 9] = [
    "net.minecraft.world.level.chunk.status.ChunkStatusTasks.buildTerrain(ChunkStatusTasks.java:82)",
    "net.minecraft.world.level.chunk.status.ChunkPyramid$$Lambda/0x00000000abec3870.doWork(Unknown Source)",
    "net.minecraft.world.level.chunk.status.ChunkStep.apply(ChunkStep.java:36)",
    "net.minecraft.server.level.ChunkMap.applyStep(ChunkMap.java:666)",
    "net.minecraft.server.level.GenerationChunkHolder.applyStep(GenerationChunkHolder.java:87)",
    "net.minecraft.server.level.ChunkGenerationTask.scheduleChunkInLayer(ChunkGenerationTask.java:144)",
    "net.minecraft.server.level.ChunkGenerationTask.scheduleLayer(ChunkGenerationTask.java:121)",
    "net.minecraft.server.level.ChunkGenerationTask.scheduleNextLayer(ChunkGenerationTask.java:72)",
    "net.minecraft.server.level.ChunkGenerationTask.runUntilWait(ChunkGenerationTask.java:57)",
];

const QUERY_LISTENER_FRAMES: [&str; 9] = [
    "java.base/sun.nio.ch.Net.poll(Native Method)",
    "java.base/sun.nio.ch.DatagramChannelImpl.park(DatagramChannelImpl.java:496)",
    "java.base/sun.nio.ch.DatagramChannelImpl.tryBlockingReceive(DatagramChannelImpl.java:671)",
    "java.base/sun.nio.ch.DatagramChannelImpl.blockingReceive(DatagramChannelImpl.java:619)",
    "java.base/sun.nio.ch.DatagramSocketAdaptor.receive(DatagramSocketAdaptor.java:193)",
    "java.base/java.net.DatagramSocket.receive(DatagramSocket.java:638)",
    "net.minecraft.server.rcon.thread.QueryThreadGs4.run(QueryThreadGs4.java:257)",
    "java.base/java.lang.Thread.runWith(Thread.java:1487)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const RCON_LISTENER_FRAMES: [&str; 12] = [
    "java.base/sun.nio.ch.Net.poll(Native Method)",
    "java.base/sun.nio.ch.NioSocketImpl.park(NioSocketImpl.java:190)",
    "java.base/sun.nio.ch.NioSocketImpl.timedAccept(NioSocketImpl.java:706)",
    "java.base/sun.nio.ch.NioSocketImpl.accept(NioSocketImpl.java:748)",
    "java.base/java.net.ServerSocket.implAccept(ServerSocket.java:593)",
    "java.base/java.net.ServerSocket.platformImplAccept(ServerSocket.java:560)",
    "java.base/java.net.ServerSocket.implAccept(ServerSocket.java:536)",
    "java.base/java.net.ServerSocket.implAccept(ServerSocket.java:482)",
    "java.base/java.net.ServerSocket.accept(ServerSocket.java:441)",
    "net.minecraft.server.rcon.thread.RconThread.run(RconThread.java:42)",
    "java.base/java.lang.Thread.runWith(Thread.java:1487)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const RCON_CLIENT_READ_FRAMES: [&str; 14] = [
    "java.base/sun.nio.ch.SocketDispatcher.read0(Native Method)",
    "java.base/sun.nio.ch.SocketDispatcher.read(SocketDispatcher.java:47)",
    "java.base/sun.nio.ch.NioSocketImpl.tryRead(NioSocketImpl.java:255)",
    "java.base/sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:305)",
    "java.base/sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:354)",
    "java.base/sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:798)",
    "java.base/java.net.Socket$SocketInputStream.implRead(Socket.java:974)",
    "java.base/java.net.Socket$SocketInputStream.read(Socket.java:964)",
    "java.base/java.io.BufferedInputStream.fill(BufferedInputStream.java:289)",
    "java.base/java.io.BufferedInputStream.read1(BufferedInputStream.java:330)",
    "java.base/java.io.BufferedInputStream.read(BufferedInputStream.java:388)",
    "net.minecraft.server.rcon.thread.RconClient.run(RconClient.java:48)",
    "java.base/java.lang.Thread.runWith(Thread.java:1487)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const RCON_CLIENT_COMMAND_FRAMES: [&str; 12] = [
    "java.base/jdk.internal.misc.Unsafe.park(Native Method)",
    "java.base/java.util.concurrent.locks.LockSupport.park(LockSupport.java:223)",
    "java.base/java.util.concurrent.CompletableFuture$Signaller.block(CompletableFuture.java:1885)",
    "java.base/java.util.concurrent.ForkJoinPool.unmanagedBlock(ForkJoinPool.java:4364)",
    "java.base/java.util.concurrent.ForkJoinPool.managedBlock(ForkJoinPool.java:4310)",
    "java.base/java.util.concurrent.CompletableFuture.waitingGet(CompletableFuture.java:1919)",
    "java.base/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2138)",
    "net.minecraft.util.thread.BlockableEventLoop.executeBlocking(BlockableEventLoop.java:96)",
    "net.minecraft.server.dedicated.DedicatedServer.runCommand(DedicatedServer.java:698)",
    "net.minecraft.server.rcon.thread.RconClient.run(RconClient.java:82)",
    "java.base/java.lang.Thread.runWith(Thread.java:1487)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

fn generic_threads() -> Vec<ThreadDumpEntry> {
    crate::server::rcon::live_generic_threads()
        .iter()
        .map(|thread| {
            let running_command = thread.kind == GenericThreadKind::RconClient && thread.is_running_command();
            let (frames, locked_monitors, synchronizer): (&[&str], Vec<(usize, String)>, &str) = match thread.kind {
                GenericThreadKind::QueryListener => {
                    (&QUERY_LISTENER_FRAMES, vec![(4, "java.net.DatagramPacket@6052f905".to_string())], "java.util.concurrent.locks.ReentrantLock$NonfairSync@54a2cea")
                }
                GenericThreadKind::RconListener => (&RCON_LISTENER_FRAMES, Vec::new(), "java.util.concurrent.locks.ReentrantLock$NonfairSync@3ff23db"),
                GenericThreadKind::RconClient if running_command => (&RCON_CLIENT_COMMAND_FRAMES, Vec::new(), ""),
                GenericThreadKind::RconClient => {
                    (&RCON_CLIENT_READ_FRAMES, vec![(10, "java.io.BufferedInputStream@3c0f93f1".to_string())], "java.util.concurrent.locks.ReentrantLock$NonfairSync@20b0e9f4")
                }
            };
            ThreadDumpEntry {
                name: thread.name.clone(),
                daemon: false,
                priority: 8,
                id: thread.id,
                state: if running_command { "WAITING" } else { "RUNNABLE" },
                lock: running_command.then(|| "java.util.concurrent.CompletableFuture$Signaller@5a2bf7c2".to_string()),
                in_native: !running_command,
                frames: qualified_frames(frames),
                locked_monitors,
                locked_synchronizers: if synchronizer.is_empty() { Vec::new() } else { vec![synchronizer.to_string()] },
            }
        })
        .collect()
}

fn jvm_threads() -> Vec<ThreadDumpEntry> {
    let reference_queue_lock = |address: &str| format!("java.lang.ref.ReferenceQueue$Lock@{address}");
    vec![
        jvm_thread("DestroyJavaVM", false, 5, 104, "RUNNABLE", None, false, Vec::new()),
        jvm_thread("Attach Listener", true, 5, 14, "RUNNABLE", None, false, Vec::new()),
        jvm_thread("Notification Thread", true, 9, 32, "RUNNABLE", None, false, Vec::new()),
        jvm_thread("Signal Dispatcher", true, 9, 13, "RUNNABLE", None, false, Vec::new()),
        jvm_thread(
            "Reference Handler",
            true,
            10,
            11,
            "RUNNABLE",
            None,
            false,
            qualified_frames(&[
                "java.base/java.lang.ref.Reference.waitForReferencePendingList(Native Method)",
                "java.base/java.lang.ref.Reference.processPendingReferences(Reference.java:246)",
                "java.base/java.lang.ref.Reference$ReferenceHandler.run(Reference.java:208)",
            ]),
        ),
        jvm_thread(
            "Finalizer",
            true,
            8,
            12,
            "WAITING",
            Some(&reference_queue_lock("22f7ed47")),
            false,
            qualified_frames(&[
                "java.base/java.lang.Object.wait0(Native Method)",
                "java.base/java.lang.Object.wait(Object.java:389)",
                "java.base/java.lang.Object.wait(Object.java:351)",
                "java.base/java.lang.ref.ReferenceQueue.remove0(ReferenceQueue.java:137)",
                "java.base/java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:215)",
                "java.base/java.lang.ref.Finalizer$FinalizerThread.run(Finalizer.java:165)",
            ]),
        ),
        jvm_thread(
            "Common-Cleaner",
            true,
            8,
            33,
            "TIMED_WAITING",
            Some(&reference_queue_lock("11b355ae")),
            false,
            qualified_frames(&[
                "java.base/java.lang.Object.wait0(Native Method)",
                "java.base/java.lang.Object.wait(Object.java:389)",
                "java.base/java.lang.ref.ReferenceQueue.remove0(ReferenceQueue.java:123)",
                "java.base/java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:201)",
                "java.base/jdk.internal.ref.CleanerImpl.run(CleanerImpl.java:146)",
                "java.base/java.lang.Thread.runWith(Thread.java:1487)",
                "java.base/java.lang.Thread.run(Thread.java:1474)",
                "java.base/jdk.internal.misc.InnocuousThread.run(InnocuousThread.java:148)",
            ]),
        ),
        jvm_thread(
            "JNA Cleaner",
            true,
            5,
            52,
            "TIMED_WAITING",
            Some(&reference_queue_lock("7381c092")),
            false,
            qualified_frames(&[
                "java.base/java.lang.Object.wait0(Native Method)",
                "java.base/java.lang.Object.wait(Object.java:389)",
                "java.base/java.lang.ref.ReferenceQueue.remove0(ReferenceQueue.java:123)",
                "java.base/java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:201)",
                "com.sun.jna.internal.Cleaner$CleanerThread.run(Cleaner.java:154)",
            ]),
        ),
        jvm_thread(
            "Timer hack thread",
            true,
            5,
            64,
            "TIMED_WAITING",
            None,
            false,
            qualified_frames(&[
                "java.base/java.lang.Thread.sleepNanos0(Native Method)",
                "java.base/java.lang.Thread.sleepNanos(Thread.java:509)",
                "java.base/java.lang.Thread.sleep(Thread.java:540)",
                "net.minecraft.util.Util$9.run(Util.java:1009)",
            ]),
        ),
    ]
}

fn dump_thread_info(server_frames: &[&'static str]) -> String {
    let server_waiting = server_frames.first().is_some_and(|frame| frame.contains("Unsafe.park"));
    let mut entries = vec![
        ThreadDumpEntry {
            name: "Server thread".to_string(),
            daemon: false,
            priority: 8,
            id: SERVER_THREAD_ID,
            state: if server_waiting { "WAITING" } else { "RUNNABLE" },
            lock: server_waiting.then(|| "java.util.concurrent.CompletableFuture$Signaller@2f8d84b9".to_string()),
            in_native: false,
            frames: thread_info_frames(server_frames),
            locked_monitors: Vec::new(),
            locked_synchronizers: Vec::new(),
        },
        jvm_thread(
            "Server Watchdog",
            true,
            8,
            watchdog_id(),
            "RUNNABLE",
            None,
            false,
            qualified_frames(&[
                "java.management/sun.management.ThreadImpl.dumpThreads0(Native Method)",
                "java.management/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:505)",
                "java.management/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:493)",
                "net.minecraft.util.Util.dumpThreadInfo(Util.java:500)",
                "net.minecraft.server.dedicated.ServerWatchdog.createWatchdogCrashReport(ServerWatchdog.java:79)",
                "net.minecraft.server.dedicated.ServerWatchdog.run(ServerWatchdog.java:52)",
                "java.base/java.lang.Thread.runWith(Thread.java:1487)",
                "java.base/java.lang.Thread.run(Thread.java:1474)",
            ]),
        ),
        console_handler_thread(),
    ];
    entries.extend(netty_io_threads());
    entries.extend(jvm_threads());
    if crate::server::services::discovery_executor_blocked() {
        entries.push(services_discovery_thread());
    } else if crate::server::services::discovery_executor_started() {
        entries.push(services_discovery_idle_thread());
    }
    for (index, priority, id) in crate::server::level::chunk_workers::live_io_workers() {
        entries.push(ThreadDumpEntry {
            name: format!("IO-Worker-{index}"),
            daemon: false,
            priority,
            id,
            state: "TIMED_WAITING",
            lock: Some("java.util.concurrent.SynchronousQueue$Transferer@7e1e9134".to_string()),
            in_native: false,
            frames: idle_io_worker_frames(),
            locked_monitors: Vec::new(),
            locked_synchronizers: Vec::new(),
        });
    }
    entries.extend(worker_main_threads("java.util.concurrent.ForkJoinPool@1a2b089"));
    entries.extend(generic_threads());
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
    ThreadDumpEntry {
        name: name.to_string(),
        daemon,
        priority,
        id,
        state,
        lock: lock.map(ToString::to_string),
        in_native,
        frames,
        locked_monitors: Vec::new(),
        locked_synchronizers: Vec::new(),
    }
}

pub fn dump_threads_for_debug_report(blocked_io_worker: &str) -> String {
    let server_frames = qualified_frames(&[
        "java.management/sun.management.ThreadImpl.dumpThreads0(Native Method)",
        "java.management/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:505)",
        "java.management/sun.management.ThreadImpl.dumpAllThreads(ThreadImpl.java:493)",
        "net.minecraft.util.Util.dumpThreadInfo(Util.java:500)",
        "net.minecraft.server.MinecraftServer.dumpThreads(MinecraftServer.java:1878)",
        "net.minecraft.server.MinecraftServer.saveDebugReport(MinecraftServer.java:1833)",
        "net.minecraft.server.MinecraftServer.lambda$createProfiler$1(MinecraftServer.java:1917)",
        "net.minecraft.server.MinecraftServer$$Lambda/0x000000001f08d9f8.run(Unknown Source)",
        "net.minecraft.util.thread.BlockableEventLoop.lambda$submitAsync$0(BlockableEventLoop.java:75)",
    ]);
    let mut entries = vec![
        jvm_thread("Server thread", false, 8, SERVER_THREAD_ID, "RUNNABLE", None, false, server_frames),
        jvm_thread(
            "Server Watchdog",
            true,
            8,
            watchdog_id(),
            "TIMED_WAITING",
            None,
            false,
            qualified_frames(&[
                "java.base/java.lang.Thread.sleepNanos0(Native Method)",
                "java.base/java.lang.Thread.sleepNanos(Thread.java:509)",
                "java.base/java.lang.Thread.sleep(Thread.java:540)",
                "net.minecraft.server.dedicated.ServerWatchdog.run(ServerWatchdog.java:72)",
                "java.base/java.lang.Thread.runWith(Thread.java:1487)",
                "java.base/java.lang.Thread.run(Thread.java:1474)",
            ]),
        ),
        console_handler_thread(),
    ];
    entries.extend(netty_io_threads());
    entries.extend(jvm_threads());
    for (index, priority, id) in crate::server::level::chunk_workers::live_io_workers() {
        let name = format!("IO-Worker-{index}");
        if name == blocked_io_worker {
            entries.push(ThreadDumpEntry {
                name,
                daemon: false,
                priority,
                id,
                state: "WAITING",
                lock: Some("java.util.concurrent.CompletableFuture$Signaller@5b64b5e2".to_string()),
                in_native: false,
                frames: qualified_frames(&[
                    "java.base/jdk.internal.misc.Unsafe.park(Native Method)",
                    "java.base/java.util.concurrent.locks.LockSupport.park(LockSupport.java:223)",
                    "java.base/java.util.concurrent.CompletableFuture$Signaller.block(CompletableFuture.java:1885)",
                    "java.base/java.util.concurrent.ForkJoinPool.unmanagedBlock(ForkJoinPool.java:4364)",
                    "java.base/java.util.concurrent.ForkJoinPool.managedBlock(ForkJoinPool.java:4310)",
                    "java.base/java.util.concurrent.CompletableFuture.waitingGet(CompletableFuture.java:1919)",
                    "java.base/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2138)",
                    "net.minecraft.util.thread.BlockableEventLoop.executeBlocking(BlockableEventLoop.java:96)",
                    "net.minecraft.server.MinecraftServer.lambda$createProfiler$2(MinecraftServer.java:1912)",
                ]),
                locked_monitors: Vec::new(),
                locked_synchronizers: vec!["java.util.concurrent.ThreadPoolExecutor$Worker@7f43bd7".to_string()],
            });
        } else {
            entries.push(ThreadDumpEntry {
                name,
                daemon: false,
                priority,
                id,
                state: "TIMED_WAITING",
                lock: Some("java.util.concurrent.SynchronousQueue$Transferer@43a01550".to_string()),
                in_native: false,
                frames: idle_io_worker_frames(),
                locked_monitors: Vec::new(),
                locked_synchronizers: Vec::new(),
            });
        }
    }
    entries.extend(worker_main_threads("java.util.concurrent.ForkJoinPool@49ba0150"));
    entries.extend(generic_threads());
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
    let frames: Vec<&'static str> = thread_info_frames(&server_frames).iter().map(|frame| &*Box::leak(runtime_frame(frame).into_boxed_str())).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::command_frames::COMMAND_EXECUTION;

    #[test]
    fn command_lambdas_show_their_hidden_proxy_frame() {
        let frames = thread_info_frames(&["net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$0(FillCommand.java:84)", COMMAND_EXECUTION[0]]);
        assert_eq!(
            frames,
            [
                "net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$0(FillCommand.java:84)",
                "net.minecraft.server.commands.FillCommand$$Lambda/0x0000000032d62b30.run(Unknown Source)",
                "com.mojang.brigadier.context.ContextChain.runExecutable(ContextChain.java:73)",
            ]
        );
    }

    #[test]
    fn rcon_and_query_threads_are_listed_while_running() {
        let thread = crate::server::rcon::start_generic_thread("Server thread", "RCON Client /127.0.0.9", GenericThreadKind::RconClient);
        let dump = dump_thread_info(&[]);
        assert!(dump.contains(&format!("\"{}\" prio=8 Id={} RUNNABLE (in native)\n\tat java.base@", thread.name, thread.id)), "{dump}");
        thread.set_running_command(true);
        let dump = dump_thread_info(&[]);
        assert!(dump.contains(&format!("\"{}\" prio=8 Id={} WAITING on java.util.concurrent.CompletableFuture$Signaller@", thread.name, thread.id)), "{dump}");
        thread.finish();
        assert!(!dump_thread_info(&[]).contains(&thread.name));
    }
}
