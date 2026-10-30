use std::io;
use std::path::Path;

use crate::util::java_throwable::JavaThrowable;

pub const GET_REGION_FILE_CREATE_DIRECTORIES: &str = "net.minecraft.world.level.chunk.storage.RegionFileStorage.getRegionFile(RegionFileStorage.java:58)";
pub const GET_REGION_FILE_NEW_REGION: &str = "net.minecraft.world.level.chunk.storage.RegionFileStorage.getRegionFile(RegionFileStorage.java:60)";
pub const READ_GET_REGION_FILE: [&str; 1] = ["net.minecraft.world.level.chunk.storage.RegionFileStorage.read(RegionFileStorage.java:80)"];
pub const SCAN_CHUNK_GET_REGION_FILE: [&str; 1] = ["net.minecraft.world.level.chunk.storage.RegionFileStorage.scanChunk(RegionFileStorage.java:95)"];
pub const WRITE_GET_REGION_FILE: [&str; 2] = [
    "net.minecraft.world.level.chunk.storage.RegionFileStorage.getOrCreateRegionFile(RegionFileStorage.java:66)",
    "net.minecraft.world.level.chunk.storage.RegionFileStorage.write(RegionFileStorage.java:111)",
];

const THREAD_TAIL: [&str; 3] = [
    "java.base/java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1090)",
    "java.base/java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];
const CONSECUTIVE_EXECUTOR_TAIL: [&str; 3] = [
    "net.minecraft.util.Util.runNamed(Util.java:351)",
    "net.minecraft.util.thread.AbstractConsecutiveExecutor.pollTask(AbstractConsecutiveExecutor.java:50)",
    "net.minecraft.util.thread.AbstractConsecutiveExecutor.run(AbstractConsecutiveExecutor.java:60)",
];
const STRICT_QUEUE_RUN: &str = "net.minecraft.util.thread.StrictQueue$RunnableWithPriority.run(StrictQueue.java:50)";
const SUBMIT_THROWING_TASK: [&str; 3] = [
    "net.minecraft.world.level.chunk.storage.IOWorker.lambda$submitThrowingTask$0(IOWorker.java:247)",
    "net.minecraft.util.thread.PriorityConsecutiveExecutor.lambda$scheduleWithResult$0(PriorityConsecutiveExecutor.java:22)",
    STRICT_QUEUE_RUN,
];
const LOAD_ASYNC: &str = "net.minecraft.world.level.chunk.storage.IOWorker.lambda$loadAsync$0(IOWorker.java:196)";
const SCAN_CHUNK: &str = "net.minecraft.world.level.chunk.storage.IOWorker.lambda$scanChunk$0(IOWorker.java:233)";
const RUN_STORE: &str = "net.minecraft.world.level.chunk.storage.IOWorker.runStore(IOWorker.java:280)";
const RUN_STORE_COMPLETE: &str = "net.minecraft.world.level.chunk.storage.IOWorker.runStore(IOWorker.java:284)";
const STORE_PENDING_CHUNK: &str = "net.minecraft.world.level.chunk.storage.IOWorker.storePendingChunk(IOWorker.java:270)";
const COMPLETION_EXCEPTION: &str = "java.util.concurrent.CompletionException";
const COMPLETION_RELAY: [&str; 6] = [
    "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
    "java.base/java.util.concurrent.CompletableFuture.encodeRelay(CompletableFuture.java:412)",
    "java.base/java.util.concurrent.CompletableFuture.completeRelay(CompletableFuture.java:421)",
    "java.base/java.util.concurrent.CompletableFuture$UniRelay.tryFire(CompletableFuture.java:1118)",
    "java.base/java.util.concurrent.CompletableFuture.postComplete(CompletableFuture.java:531)",
    "java.base/java.util.concurrent.CompletableFuture.completeExceptionally(CompletableFuture.java:2221)",
];

#[cfg(windows)]
mod nio {
    pub const ACCESS_DENIED: [&str; 3] = [
        "java.base/sun.nio.fs.WindowsException.translateToIOException(WindowsException.java:89)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:103)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:108)",
    ];
    pub const FILE_EXISTS: [&str; 3] = [
        "java.base/sun.nio.fs.WindowsException.translateToIOException(WindowsException.java:87)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:103)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:108)",
    ];
    pub const NO_SUCH_FILE: [&str; 3] = [
        "java.base/sun.nio.fs.WindowsException.translateToIOException(WindowsException.java:85)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:103)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:108)",
    ];
    pub const FILE_SYSTEM: [&str; 3] = [
        "java.base/sun.nio.fs.WindowsException.translateToIOException(WindowsException.java:92)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:103)",
        "java.base/sun.nio.fs.WindowsException.rethrowAsIOException(WindowsException.java:108)",
    ];
    pub const NEW_FILE_CHANNEL: [&str; 3] = [
        "java.base/sun.nio.fs.WindowsFileSystemProvider.newFileChannel(WindowsFileSystemProvider.java:118)",
        "java.base/java.nio.channels.FileChannel.open(FileChannel.java:301)",
        "java.base/java.nio.channels.FileChannel.open(FileChannel.java:353)",
    ];
    pub const NEW_INPUT_STREAM: [&str; 5] = [
        "java.base/sun.nio.fs.WindowsFileSystemProvider.newByteChannel(WindowsFileSystemProvider.java:231)",
        "java.base/java.nio.file.Files.newByteChannel(Files.java:357)",
        "java.base/java.nio.file.Files.newByteChannel(Files.java:399)",
        "java.base/java.nio.file.spi.FileSystemProvider.newInputStream(FileSystemProvider.java:371)",
        "java.base/java.nio.file.Files.newInputStream(Files.java:154)",
    ];
    pub const NEW_OUTPUT_STREAM: [&str; 3] = [
        "java.base/sun.nio.fs.WindowsFileSystemProvider.newByteChannel(WindowsFileSystemProvider.java:231)",
        "java.base/java.nio.file.spi.FileSystemProvider.newOutputStream(FileSystemProvider.java:426)",
        "java.base/java.nio.file.Files.newOutputStream(Files.java:215)",
    ];
    pub const CREATE_DIRECTORIES: [&str; 4] = [
        "java.base/sun.nio.fs.WindowsFileSystemProvider.createDirectory(WindowsFileSystemProvider.java:503)",
        "java.base/java.nio.file.Files.createDirectory(Files.java:647)",
        "java.base/java.nio.file.Files.createAndCheckIsDirectory(Files.java:734)",
        "java.base/java.nio.file.Files.createDirectories(Files.java:689)",
    ];
}

#[cfg(not(windows))]
mod nio {
    pub const ACCESS_DENIED: [&str; 3] = [
        "java.base/sun.nio.fs.UnixException.translateToIOException(UnixException.java:90)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:106)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:111)",
    ];
    pub const FILE_EXISTS: [&str; 3] = [
        "java.base/sun.nio.fs.UnixException.translateToIOException(UnixException.java:94)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:106)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:111)",
    ];
    pub const NO_SUCH_FILE: [&str; 3] = [
        "java.base/sun.nio.fs.UnixException.translateToIOException(UnixException.java:92)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:106)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:111)",
    ];
    pub const FILE_SYSTEM: [&str; 3] = [
        "java.base/sun.nio.fs.UnixException.translateToIOException(UnixException.java:100)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:106)",
        "java.base/sun.nio.fs.UnixException.rethrowAsIOException(UnixException.java:111)",
    ];
    pub const NEW_FILE_CHANNEL: [&str; 3] = [
        "java.base/sun.nio.fs.UnixFileSystemProvider.newFileChannel(UnixFileSystemProvider.java:213)",
        "java.base/java.nio.channels.FileChannel.open(FileChannel.java:301)",
        "java.base/java.nio.channels.FileChannel.open(FileChannel.java:353)",
    ];
    pub const NEW_INPUT_STREAM: [&str; 5] = [
        "java.base/sun.nio.fs.UnixFileSystemProvider.newByteChannel(UnixFileSystemProvider.java:261)",
        "java.base/java.nio.file.Files.newByteChannel(Files.java:357)",
        "java.base/java.nio.file.Files.newByteChannel(Files.java:399)",
        "java.base/java.nio.file.spi.FileSystemProvider.newInputStream(FileSystemProvider.java:371)",
        "java.base/java.nio.file.Files.newInputStream(Files.java:154)",
    ];
    pub const NEW_OUTPUT_STREAM: [&str; 3] = [
        "java.base/sun.nio.fs.UnixFileSystemProvider.newByteChannel(UnixFileSystemProvider.java:261)",
        "java.base/java.nio.file.spi.FileSystemProvider.newOutputStream(FileSystemProvider.java:426)",
        "java.base/java.nio.file.Files.newOutputStream(Files.java:215)",
    ];
    pub const CREATE_DIRECTORIES: [&str; 4] = [
        "java.base/sun.nio.fs.UnixFileSystemProvider.createDirectory(UnixFileSystemProvider.java:418)",
        "java.base/java.nio.file.Files.createDirectory(Files.java:647)",
        "java.base/java.nio.file.Files.createAndCheckIsDirectory(Files.java:734)",
        "java.base/java.nio.file.Files.createDirectories(Files.java:689)",
    ];
}

const CREATE_DIRECTORIES_SAFE: &str = "net.minecraft.util.FileUtil.createDirectoriesSafe(FileUtil.java:185)";
const REGION_FILE_NEW_SYNC: &str = "net.minecraft.world.level.chunk.storage.RegionFile.<init>(RegionFile.java:132)";
const REGION_FILE_NEW: &str = "net.minecraft.world.level.chunk.storage.RegionFile.<init>(RegionFile.java:134)";
const REGION_FILE_NEW_DELEGATE: &str = "net.minecraft.world.level.chunk.storage.RegionFile.<init>(RegionFile.java:115)";

fn syscall_frames(error: &io::Error) -> [&'static str; 3] {
    match error.kind() {
        io::ErrorKind::NotFound => nio::NO_SUCH_FILE,
        io::ErrorKind::PermissionDenied => nio::ACCESS_DENIED,
        io::ErrorKind::AlreadyExists => nio::FILE_EXISTS,
        _ => nio::FILE_SYSTEM,
    }
}

pub const OPTIONAL_OR_ELSE_THROW: &str = "java.base/java.util.Optional.orElseThrow(Optional.java:377)";
pub const ENTITY_STORAGE_POSITION_FRAME: &str = "net.minecraft.world.level.chunk.storage.EntityStorage.lambda$loadEntities$0(EntityStorage.java:66)";
pub const ENTITY_STORAGE_MISPLACED_FRAME: &str = "net.minecraft.world.level.chunk.storage.EntityStorage.lambda$loadEntities$0(EntityStorage.java:69)";
pub const READ_CHUNK_STREAM: &str = "net.minecraft.world.level.chunk.storage.RegionFileStorage.read(RegionFileStorage.java:90)";
pub const SCAN_CHUNK_STREAM: &str = "net.minecraft.world.level.chunk.storage.RegionFileStorage.scanChunk(RegionFileStorage.java:102)";


pub fn construct_stream_caller(caller: &'static str) -> &'static str {
    if caller == SCAN_CHUNK_STREAM { "net.minecraft.world.level.chunk.storage.RegionFileStorage.scanChunk(RegionFileStorage.java:100)" } else { "net.minecraft.world.level.chunk.storage.RegionFileStorage.read(RegionFileStorage.java:85)" }
}

pub fn java_path(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\").map_or(text.clone(), str::to_string)
}

pub fn create_directories_safe_failure(error: io::Error, directory: &Path, callers: &[&'static str]) -> io::Error {
    let kind = error.kind();
    let frames = syscall_frames(&error);
    let mut throwable = path_throwable(error, directory);
    throwable.stack_trace.extend(frames);
    throwable.stack_trace.extend(nio::CREATE_DIRECTORIES);
    throwable.stack_trace.push(CREATE_DIRECTORIES_SAFE);
    throwable.stack_trace.push(GET_REGION_FILE_CREATE_DIRECTORIES);
    throwable.stack_trace.extend_from_slice(callers);
    io::Error::new(kind, throwable)
}

pub fn open_region_file_failure(error: io::Error, path: &Path, sync: bool, callers: &[&'static str]) -> io::Error {
    let kind = error.kind();
    let frames = syscall_frames(&error);
    let mut throwable = path_throwable(error, path);
    throwable.stack_trace.extend(frames);
    throwable.stack_trace.extend(nio::NEW_FILE_CHANNEL);
    throwable.stack_trace.push(if sync { REGION_FILE_NEW_SYNC } else { REGION_FILE_NEW });
    throwable.stack_trace.push(REGION_FILE_NEW_DELEGATE);
    throwable.stack_trace.push(GET_REGION_FILE_NEW_REGION);
    throwable.stack_trace.extend_from_slice(callers);
    io::Error::new(kind, throwable)
}

pub fn new_input_stream_failure(error: io::Error, path: &Path) -> JavaThrowable {
    let frames = syscall_frames(&error);
    let mut throwable = path_throwable(error, path);
    throwable.stack_trace.extend(frames);
    throwable.stack_trace.extend(nio::NEW_INPUT_STREAM);
    throwable
}

pub fn new_output_stream_failure(error: io::Error, path: &Path) -> JavaThrowable {
    let frames = syscall_frames(&error);
    let mut throwable = path_throwable(error, path);
    throwable.stack_trace.extend(frames);
    throwable.stack_trace.extend(nio::NEW_OUTPUT_STREAM);
    throwable
}

fn path_throwable(error: io::Error, path: &Path) -> JavaThrowable {
    JavaThrowable::from_io_error(&crate::util::java_nio::for_path(error, Path::new(&java_path(path))))
}


fn on_io_worker_thread(error: &io::Error, task: &[&'static str]) -> JavaThrowable {
    let mut throwable = JavaThrowable::from_io_error(error);
    let mut tail = task.to_vec();
    tail.extend(CONSECUTIVE_EXECUTOR_TAIL);
    tail.extend(THREAD_TAIL);
    throwable.push_frames(&tail);
    throwable
}

pub fn on_load_worker(error: &io::Error) -> JavaThrowable {
    let [submit, schedule, run] = SUBMIT_THROWING_TASK;
    on_io_worker_thread(error, &[LOAD_ASYNC, submit, schedule, run])
}

pub fn is_java_error(error: &io::Error) -> bool {
    matches!(JavaThrowable::from_io_error(error).class_name(), "java.lang.OutOfMemoryError" | "java.lang.StackOverflowError")
}

pub fn on_load_worker_uncaught(error: &io::Error) -> JavaThrowable {
    let mut throwable = JavaThrowable::from_io_error(error);
    let [submit, schedule, run] = SUBMIT_THROWING_TASK;
    let mut tail = vec![
        LOAD_ASYNC,
        "net.minecraft.world.level.chunk.storage.IOWorker$$Lambda/0x0000000019019a78.get(Unknown Source)",
        submit,
        "net.minecraft.world.level.chunk.storage.IOWorker$$Lambda/0x0000000019019ca8.accept(Unknown Source)",
        schedule,
        "net.minecraft.util.thread.PriorityConsecutiveExecutor$$Lambda/0x0000000019019ee8.run(Unknown Source)",
        run,
    ];
    tail.extend(CONSECUTIVE_EXECUTOR_TAIL);
    tail.extend(&THREAD_TAIL[..2]);
    tail.push("java.base/java.lang.Thread.runWith(Thread.java:1487)");
    tail.push(THREAD_TAIL[2]);
    throwable.push_frames(&tail);
    throwable
}

pub fn on_scan_worker(error: &io::Error) -> JavaThrowable {
    let [submit, schedule, run] = SUBMIT_THROWING_TASK;
    on_io_worker_thread(error, &[SCAN_CHUNK, submit, schedule, run])
}

pub fn store_worker_frames() -> Vec<&'static str> {
    let mut frames = vec![RUN_STORE, STORE_PENDING_CHUNK, STRICT_QUEUE_RUN];
    frames.extend(CONSECUTIVE_EXECUTOR_TAIL);
    frames.extend(THREAD_TAIL);
    frames
}

pub fn on_store_worker(error: &io::Error) -> JavaThrowable {
    on_io_worker_thread(error, &[RUN_STORE, STORE_PENDING_CHUNK, STRICT_QUEUE_RUN])
}

pub fn structure_check_join_failure(cause: &JavaThrowable) -> JavaThrowable {
    let frames = vec![
        "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
        "java.base/java.util.concurrent.CompletableFuture.reportJoin(CompletableFuture.java:457)",
        "java.base/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2139)",
        "net.minecraft.world.level.levelgen.structure.StructureCheck.tryLoadFromStorage(StructureCheck.java:143)",
    ];
    JavaThrowable::new(format!("{COMPLETION_EXCEPTION}: {}", cause.description), frames).with_cause(cause.clone())
}

pub fn section_storage_completion_exception(cause: &JavaThrowable) -> JavaThrowable {
    let mut frames = vec![
        "net.minecraft.world.level.chunk.storage.SectionStorage.lambda$tryRead$2(SectionStorage.java:195)",
        "java.base/java.util.concurrent.CompletableFuture.uniExceptionally(CompletableFuture.java:1011)",
        "java.base/java.util.concurrent.CompletableFuture$UniExceptionally.tryFire(CompletableFuture.java:995)",
        "java.base/java.util.concurrent.CompletableFuture.postComplete(CompletableFuture.java:531)",
        "java.base/java.util.concurrent.CompletableFuture.completeExceptionally(CompletableFuture.java:2221)",
        "net.minecraft.world.level.chunk.storage.IOWorker.lambda$submitThrowingTask$0(IOWorker.java:249)",
        "net.minecraft.util.thread.PriorityConsecutiveExecutor.lambda$scheduleWithResult$0(PriorityConsecutiveExecutor.java:22)",
        STRICT_QUEUE_RUN,
    ];
    frames.extend(CONSECUTIVE_EXECUTOR_TAIL);
    frames.extend(THREAD_TAIL);
    JavaThrowable::new(format!("{COMPLETION_EXCEPTION}: {}", cause.description), frames).with_cause(cause.clone())
}

pub fn entity_manager_completion_exception(cause: &JavaThrowable) -> JavaThrowable {
    let mut frames = vec![
        "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
        "java.base/java.util.concurrent.CompletableFuture.encodeThrowable(CompletableFuture.java:376)",
        "java.base/java.util.concurrent.CompletableFuture.completeThrowable(CompletableFuture.java:391)",
        "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:657)",
        "java.base/java.util.concurrent.CompletableFuture.postComplete(CompletableFuture.java:531)",
        "java.base/java.util.concurrent.CompletableFuture.completeExceptionally(CompletableFuture.java:2221)",
        "net.minecraft.world.level.chunk.storage.IOWorker.lambda$submitThrowingTask$0(IOWorker.java:249)",
        "net.minecraft.util.thread.PriorityConsecutiveExecutor.lambda$scheduleWithResult$0(PriorityConsecutiveExecutor.java:22)",
        STRICT_QUEUE_RUN,
    ];
    frames.extend(CONSECUTIVE_EXECUTOR_TAIL);
    frames.extend(THREAD_TAIL);
    JavaThrowable::new(format!("{COMPLETION_EXCEPTION}: {}", cause.description), frames).with_cause(cause.clone())
}

pub fn scan_join_failure(cause: &JavaThrowable) -> JavaThrowable {
    let frames = vec![
        "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
        "java.base/java.util.concurrent.CompletableFuture.reportJoin(CompletableFuture.java:457)",
        "java.base/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2139)",
        "net.minecraft.world.level.chunk.storage.IOWorker.lambda$createOldDataForRegion$1(IOWorker.java:148)",
        "net.minecraft.world.level.ChunkPos$2.tryAdvance(ChunkPos.java:245)",
        "java.base/java.util.Spliterator.forEachRemaining(Spliterator.java:332)",
        "java.base/java.util.stream.ReferencePipeline$Head.forEach(ReferencePipeline.java:803)",
        "net.minecraft.world.level.chunk.storage.IOWorker.lambda$createOldDataForRegion$0(IOWorker.java:141)",
        "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)",
        "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.exec(CompletableFuture.java:1781)",
        "java.base/java.util.concurrent.ForkJoinTask.doExec(ForkJoinTask.java:511)",
        "java.base/java.util.concurrent.ForkJoinPool$WorkQueue.topLevelExec(ForkJoinPool.java:1450)",
        "java.base/java.util.concurrent.ForkJoinPool.runWorker(ForkJoinPool.java:2019)",
        "java.base/java.util.concurrent.ForkJoinWorkerThread.run(ForkJoinWorkerThread.java:187)",
    ];
    JavaThrowable::new(format!("{COMPLETION_EXCEPTION}: {}", cause.description), frames).with_cause(cause.clone())
}

pub fn completion_exception_of_store(store_failure: JavaThrowable) -> JavaThrowable {
    let mut frames: Vec<&'static str> = COMPLETION_RELAY.to_vec();
    frames.extend([RUN_STORE_COMPLETE, STORE_PENDING_CHUNK, STRICT_QUEUE_RUN]);
    frames.extend(CONSECUTIVE_EXECUTOR_TAIL);
    frames.extend(THREAD_TAIL);
    JavaThrowable::new(format!("{COMPLETION_EXCEPTION}: {}", store_failure.description), frames).with_cause(store_failure)
}
