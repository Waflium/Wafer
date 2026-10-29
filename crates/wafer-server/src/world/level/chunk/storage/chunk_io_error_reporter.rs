use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};

use wafer_core::ChunkPos;

use super::region_storage_info::RegionStorageInfo;
use crate::crash_report::{CrashReport, ReportType};
use crate::server::suppressed_exception_collector::SuppressedExceptionCollector;
use crate::util::java_throwable::{JavaThrowable, ThrowableReport};

const STORE_CHUNK_IO_ERROR_FRAMES: [&str; 4] = [
    "net.minecraft.server.MinecraftServer.lambda$storeChunkIoError$0(MinecraftServer.java:2225)",
    "java.base/java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1090)",
    "java.base/java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];
const CREATE_MISPLACED_CHUNK_REPORT_FRAME: &str = "net.minecraft.world.level.chunk.storage.ChunkIOErrorReporter.createMisplacedChunkReport(ChunkIOErrorReporter.java:16)";
const REPORT_MISPLACED_CHUNK_FRAME: &str = "net.minecraft.world.level.chunk.storage.ChunkIOErrorReporter.reportMisplacedChunk(ChunkIOErrorReporter.java:24)";
const CHUNK_MAP_LOAD_FRAMES: [&str; 2] = [
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.read(SerializableChunkData.java:226)",
    "net.minecraft.server.level.ChunkMap.lambda$scheduleChunkLoad$3(ChunkMap.java:597)",
];
const UNI_APPLY_FRAMES: [&str; 2] = [
    "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:667)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.run(CompletableFuture.java:503)",
];
const MISPLACED_CHUNK_REPORTED_FRAME: &str = "net.minecraft.world.level.chunk.storage.ChunkIOErrorReporter.createMisplacedChunkReport(ChunkIOErrorReporter.java:19)";
const MAIN_THREAD_TASK_FRAMES: [&str; 4] = [
    "net.minecraft.util.thread.BlockableEventLoop.doRunTask(BlockableEventLoop.java:173)",
    "net.minecraft.server.level.ServerChunkCache$MainThreadExecutor.doRunTask(ServerChunkCache.java:621)",
    "net.minecraft.util.thread.BlockableEventLoop.pollTask(BlockableEventLoop.java:147)",
    "net.minecraft.server.level.ServerChunkCache$MainThreadExecutor.pollTask(ServerChunkCache.java:630)",
];
const MAIN_THREAD_POLL_FRAMES: [&str; 3] = [
    "net.minecraft.server.level.ServerChunkCache.pollTask(ServerChunkCache.java:268)",
    "net.minecraft.server.MinecraftServer.pollTaskInternal(MinecraftServer.java:900)",
    "net.minecraft.server.MinecraftServer.pollTask(MinecraftServer.java:888)",
];
const PREPARE_LEVELS_FRAMES: [&str; 8] = [
    "net.minecraft.util.thread.BlockableEventLoop.managedBlock(BlockableEventLoop.java:156)",
    "net.minecraft.server.MinecraftServer.waitUntilNextTick(MinecraftServer.java:859)",
    "net.minecraft.server.MinecraftServer.prepareLevels(MinecraftServer.java:578)",
    "net.minecraft.server.MinecraftServer.loadLevel(MinecraftServer.java:411)",
    "net.minecraft.server.dedicated.DedicatedServer.initServer(DedicatedServer.java:221)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:720)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];
const RUNNING_FRAMES: [&str; 5] = [
    "net.minecraft.util.thread.BlockableEventLoop.runAllTasks(BlockableEventLoop.java:127)",
    "net.minecraft.server.MinecraftServer.waitUntilNextTick(MinecraftServer.java:856)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:758)",
    "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];
const LOW_DISK_SPACE_REPORT_BYTES: u64 = 8192;
const LOW_DISK_SPACE_WARNING_BYTES: u64 = 67_108_864;

type StoreJob = Box<dyn FnOnce() + Send>;

struct Shared {
    suppressed_exceptions: SuppressedExceptionCollector,
    server_directory: PathBuf,
    level_directory: Option<PathBuf>,
    store_jobs: Sender<StoreJob>,
    running: AtomicBool,
}

#[derive(Clone)]
pub struct ChunkIoErrorReporter {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for ChunkIoErrorReporter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ChunkIoErrorReporter")
    }
}

pub fn chunk_load_failure_cause(throwable: &JavaThrowable) -> &JavaThrowable {
    let unwrapped = if throwable.class_name() == "java.util.concurrent.CompletionException" { throwable.cause.as_deref().unwrap_or(throwable) } else { throwable };
    match (&unwrapped.report, unwrapped.cause.as_deref()) {
        (Some(_), Some(cause)) => cause,
        _ => unwrapped,
    }
}

impl ChunkIoErrorReporter {
    pub fn new(server_directory: PathBuf, level_directory: Option<PathBuf>) -> ChunkIoErrorReporter {
        let (store_jobs, jobs) = channel::<StoreJob>();
        let _ = std::thread::Builder::new().name(crate::server::level::chunk_workers::next_io_worker_name()).spawn(move || {
            for job in jobs {
                job();
            }
        });
        ChunkIoErrorReporter {
            shared: Arc::new(Shared { suppressed_exceptions: SuppressedExceptionCollector::default(), server_directory, level_directory, store_jobs, running: AtomicBool::new(false) }),
        }
    }

    pub fn suppressed_exceptions(&self) -> &SuppressedExceptionCollector {
        &self.shared.suppressed_exceptions
    }

    pub fn set_running(&self) {
        self.shared.running.store(true, Ordering::Relaxed);
    }

    fn store_chunk_io_error(&self, mut report: CrashReport, pos: ChunkPos, storage_info: &RegionStorageInfo) {
        let debug_directory = self.shared.server_directory.join("debug");
        let storage_info = storage_info.clone();
        let _ = self.shared.store_jobs.send(Box::new(move || {
            if let Err(e) = store_report(&mut report, &debug_directory, pos, &storage_info) {
                warn!("Failed to store chunk IO exception\n{}", e.print_stack_trace().trim_end());
            }
        }));
    }

    pub fn report_chunk_load_failure(&self, throwable: &JavaThrowable, storage_info: &RegionStorageInfo, pos: ChunkPos) {
        error!("Failed to load chunk {},{}\n{}", pos.x, pos.z, throwable.print_stack_trace().trim_end());
        self.shared.suppressed_exceptions.add_entry("chunk/load", throwable);
        self.store_chunk_io_error(CrashReport::for_throwable(throwable, "Chunk load failure"), pos, storage_info);
        self.warn_on_low_disk_space();
    }

    pub fn report_chunk_save_failure(&self, throwable: &JavaThrowable, storage_info: &RegionStorageInfo, pos: ChunkPos) {
        error!("Failed to save chunk {},{}\n{}", pos.x, pos.z, throwable.print_stack_trace().trim_end());
        self.shared.suppressed_exceptions.add_entry("chunk/save", throwable);
        self.store_chunk_io_error(CrashReport::for_throwable(throwable, "Chunk save failure"), pos, storage_info);
        self.warn_on_low_disk_space();
    }

    pub fn chunk_map_load_frames(&self) -> Vec<&'static str> {
        let mut frames = CHUNK_MAP_LOAD_FRAMES.to_vec();
        frames.extend(UNI_APPLY_FRAMES);
        frames.extend(MAIN_THREAD_TASK_FRAMES);
        frames.extend(MAIN_THREAD_POLL_FRAMES);
        if self.shared.running.load(Ordering::Relaxed) {
            frames.extend(RUNNING_FRAMES);
        } else {
            frames.extend(PREPARE_LEVELS_FRAMES);
        }
        frames
    }

    pub fn report_misplaced_chunk(&self, stored_pos: ChunkPos, requested_pos: ChunkPos, storage_info: &RegionStorageInfo, caller_frames: Vec<&'static str>) {
        let mut frames = vec![CREATE_MISPLACED_CHUNK_REPORT_FRAME, REPORT_MISPLACED_CHUNK_FRAME];
        frames.extend(caller_frames);
        self.report_chunk_load_failure(&misplaced_chunk_exception(stored_pos, requested_pos, frames), storage_info, requested_pos);
    }

    pub fn warn_on_low_disk_space(&self) {
        if self.check_for_low_disk_space() {
            self.send_low_disk_space_warning();
        }
    }

    fn check_for_low_disk_space(&self) -> bool {
        self.estimate_disk_space() < LOW_DISK_SPACE_WARNING_BYTES
    }

    fn estimate_disk_space(&self) -> u64 {
        self.shared.level_directory.as_ref().and_then(|directory| fs4::available_space(directory).ok()).unwrap_or(u64::MAX)
    }

    pub fn send_low_disk_space_warning(&self) {
        warn!("Low disk space! Might not be able to save the world.");
    }
}

fn misplaced_chunk_exception(stored_pos: ChunkPos, requested_pos: ChunkPos, frames: Vec<&'static str>) -> JavaThrowable {
    let illegal_state = JavaThrowable::new(format!("java.lang.IllegalStateException: Retrieved chunk position {stored_pos} does not match requested {requested_pos}"), frames.clone());
    let mut reported_frames = frames.clone();
    reported_frames[0] = MISPLACED_CHUNK_REPORTED_FRAME;
    let report = ThrowableReport {
        title: "Chunk found in invalid location".to_string(),
        category: "Misplaced Chunk".to_string(),
        details: vec![("Stored Position".to_string(), stored_pos.to_string())],
        category_stack_trace: frames,
    };
    JavaThrowable::new("net.minecraft.ReportedException: Chunk found in invalid location", reported_frames).with_cause(illegal_state).with_report(report)
}

fn store_report(report: &mut CrashReport, debug_directory: &Path, pos: ChunkPos, storage_info: &RegionStorageInfo) -> Result<(), JavaThrowable> {
    let to_throwable = |e: std::io::Error, path: &Path| JavaThrowable::from_io_error(&crate::util::java_nio::for_path(e, path));
    crate::util::file_util::create_directories_safe(debug_directory).map_err(|e| to_throwable(e, debug_directory))?;
    let sanitized_level_name = crate::util::file_util::sanitize_name(&storage_info.level);
    let report_file = debug_directory.join(format!("chunk-{}-{}-server.txt", sanitized_level_name, crate::util::get_filename_formatted_date_time()));
    let remaining_space = fs4::available_space(debug_directory).map_err(|e| to_throwable(e, debug_directory))?;
    if remaining_space < LOW_DISK_SPACE_REPORT_BYTES {
        warn!("Not storing chunk IO report due to low space on drive {}", file_store_name(debug_directory));
        return Ok(());
    }
    let category = report.add_category_with_stack_trace("Chunk Info", STORE_CHUNK_IO_ERROR_FRAMES.iter().map(|frame| frame.to_string()).collect());
    category.set_detail("Level", storage_info.level.clone());
    category.set_detail("Dimension", storage_info.dimension.clone());
    category.set_detail("Storage", storage_info.type_.clone());
    category.set_detail("Position", pos.to_string());
    report.save_to_file(&report_file, &ReportType::CHUNK_IO_ERROR);
    info!("Saved details to {}", report.get_save_file().map_or("null".to_string(), |file| file.display().to_string()));
    Ok(())
}

#[cfg(windows)]
fn file_store_name(directory: &Path) -> String {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};
    let path: Vec<u16> = std::path::absolute(directory).unwrap_or_else(|_| directory.to_path_buf()).as_os_str().encode_wide().chain([0]).collect();
    let mut root = [0u16; 261];
    let mut label = [0u16; 261];
    let found = unsafe {
        GetVolumePathNameW(path.as_ptr(), root.as_mut_ptr(), root.len() as u32) != 0
            && GetVolumeInformationW(root.as_ptr(), label.as_mut_ptr(), label.len() as u32, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0) != 0
    };
    if !found {
        return String::new();
    }
    let length = label.iter().position(|&unit| unit == 0).unwrap_or(label.len());
    std::ffi::OsString::from_wide(&label[..length]).to_string_lossy().into_owned()
}

#[cfg(unix)]
fn file_store_name(directory: &Path) -> String {
    let path = std::fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf());
    let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
    mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            let device = fields.next()?;
            let mount_point = fields.next()?.replace("\\040", " ");
            path.starts_with(&mount_point).then(|| (mount_point.len(), device.to_string()))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, device)| device)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for_report(directory: &Path) -> String {
        for _ in 0..200 {
            if let Some(file) = std::fs::read_dir(directory.join("debug")).ok().and_then(|entries| entries.flatten().next()) {
                let text = std::fs::read_to_string(file.path()).unwrap_or_default();
                if text.contains("-- System Details --") {
                    return text;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("no chunk io report in {}", directory.display());
    }

    fn temp_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("wafer-chunk-io-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn stores_a_chunk_io_report_with_head_and_chunk_info() {
        let directory = temp_directory("load");
        let reporter = ChunkIoErrorReporter::new(directory.clone(), None);
        let throwable = JavaThrowable::new(
            "java.util.zip.ZipException: incorrect header check",
            vec!["a.B.read(B.java:1)", "a.B.run(B.java:2)", "t.Pool.runWorker(P.java:3)", "t.Pool$Worker.run(P.java:4)", "java.lang.Thread.run(Thread.java:5)", "x.y(Z.java:6)"],
        );
        reporter.report_chunk_load_failure(&throwable, &RegionStorageInfo::new("my world", "minecraft:the_nether", "poi"), ChunkPos::new(-3, 7));
        let report = wait_for_report(&directory);
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines[0], "---- Minecraft Chunk IO Error Report ----");
        assert!(lines[1].starts_with("// "));
        assert!(report.contains("\nDescription: Chunk load failure\n"));
        assert!(report.contains("-- Chunk Info --\nDetails:\n\tLevel: my world\n\tDimension: minecraft:the_nether\n\tStorage: poi\n\tPosition: [-3, 7]\nStacktrace:\n\tat net.minecraft.server.MinecraftServer.lambda$storeChunkIoError$0(MinecraftServer.java:2225)"));
        assert!(report.contains("-- Head --\nThread: IO-Worker-"));
        assert!(report.contains("java.util.zip.ZipException: incorrect header check"));
        let dump = reporter.suppressed_exceptions().dump();
        assert!(dump.contains("chunk/load:class java.util.zip.ZipException: incorrect header check"), "{dump}");
        let file_name = std::fs::read_dir(directory.join("debug")).unwrap().flatten().next().unwrap().file_name().into_string().unwrap();
        assert!(file_name.starts_with("chunk-my world-") && file_name.ends_with("-server.txt"), "{file_name}");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn misplaced_chunks_use_the_reported_exception_report() {
        let directory = temp_directory("misplaced");
        let reporter = ChunkIoErrorReporter::new(directory.clone(), None);
        reporter.report_misplaced_chunk(ChunkPos::new(1, 0), ChunkPos::new(0, 0), &RegionStorageInfo::new("world", "minecraft:overworld", "chunk"), reporter.chunk_map_load_frames());
        let report = wait_for_report(&directory);
        assert!(report.contains("\nDescription: Chunk found in invalid location\n"));
        assert!(report.contains("java.lang.IllegalStateException: Retrieved chunk position [1, 0] does not match requested [0, 0]"));
        assert!(report.contains("-- Misplaced Chunk --\nDetails:\n\tStored Position: [1, 0]\nStacktrace:\n\tat net.minecraft.world.level.chunk.storage.ChunkIOErrorReporter.createMisplacedChunkReport(ChunkIOErrorReporter.java:16)"));
        assert!(reporter.suppressed_exceptions().dump().contains("chunk/load:class net.minecraft.ReportedException: Chunk found in invalid location"));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn unwraps_completion_exceptions_and_reported_exceptions_like_chunk_map() {
        let io = JavaThrowable::new("java.io.EOFException", Vec::new());
        let report = ThrowableReport { title: "Loading NBT data".to_string(), category: "NBT Tag".to_string(), details: Vec::new(), category_stack_trace: Vec::new() };
        let reported = JavaThrowable::new("net.minecraft.nbt.ReportedNbtException: Loading NBT data", Vec::new()).with_cause(io.clone()).with_report(report);
        let completion = JavaThrowable::new("java.util.concurrent.CompletionException: x", Vec::new()).with_cause(reported.clone());
        assert_eq!(chunk_load_failure_cause(&completion).description, "java.io.EOFException");
        assert_eq!(chunk_load_failure_cause(&io).description, "java.io.EOFException");
        let reported_without_report = JavaThrowable::new("net.minecraft.ReportedException: x", Vec::new()).with_cause(io.clone());
        assert_eq!(chunk_load_failure_cause(&reported_without_report).description, reported_without_report.description);
    }

    #[test]
    fn low_disk_space_thresholds_match_vanilla() {
        assert_eq!(LOW_DISK_SPACE_REPORT_BYTES, 8192);
        assert_eq!(LOW_DISK_SPACE_WARNING_BYTES, 64 * 1024 * 1024);
    }
}
