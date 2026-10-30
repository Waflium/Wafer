use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::thread;

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use wafer_data::{Block, Fluid};

use super::chunk_level::ChunkStatus;
use crate::world::level::chunk::{ChunkStructures, LevelChunk};
use crate::world::level::chunk::serialization::{self, ReadChunk, SectionLight};
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::chunk::storage::chunk_io_error_reporter::ChunkIoErrorReporter;
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;
use crate::world::level::chunk::storage::storage_exception;
use crate::world::ticks::SavedTick;

type Job = Box<dyn FnOnce() + Send>;

static IO_WORKER_COUNT: AtomicUsize = AtomicUsize::new(1);

pub fn next_io_worker_name() -> String {
    format!("IO-Worker-{}", IO_WORKER_COUNT.fetch_add(1, Ordering::Relaxed))
}

pub fn io_worker_count() -> usize {
    IO_WORKER_COUNT.load(Ordering::Relaxed) - 1
}

pub fn worker_main_count() -> usize {
    WORKER_NAMES.len()
}

struct WorkerPool {
    jobs: Mutex<VecDeque<Job>>,
    available: Condvar,
}

static POOL: LazyLock<&'static WorkerPool> = LazyLock::new(|| {
    let pool: &'static WorkerPool = Box::leak(Box::new(WorkerPool { jobs: Mutex::new(VecDeque::new()), available: Condvar::new() }));
    let threads = thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(1).clamp(1, 255);
    for i in 0..threads {
        thread::Builder::new()
            .name(format!("Worker-Main-{}", i + 1))
            .spawn(move || {
                loop {
                    let job = {
                        let mut jobs = pool.jobs.lock().unwrap();
                        loop {
                            if let Some(job) = jobs.pop_front() {
                                break job;
                            }
                            jobs = pool.available.wait(jobs).unwrap();
                        }
                    };
                    if let Err(exception) = crate::util::catch_exception(job) {
                        let exception = format!("java.util.concurrent.CompletionException: {exception}");
                        crate::crash_report::relay_delay_crash(crate::crash_report::CrashReport::new("Exception chunk generation/loading", exception, Vec::new()));
                    }
                }
            })
            .expect("worker thread");
    }
    pool
});

static WORKER_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let threads = thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(1).clamp(1, 255);
    (1..=threads).map(|i| &*Box::leak(format!("Worker-Main-{i}").into_boxed_str())).collect()
});

pub fn random_worker_name() -> &'static str {
    WORKER_NAMES[rand::random::<u32>() as usize % WORKER_NAMES.len()]
}

pub fn execute(job: impl FnOnce() + Send + 'static) {
    let pool = *POOL;
    pool.jobs.lock().unwrap().push_back(Box::new(job));
    pool.available.notify_one();
}

pub enum WorkResult {
    Loaded { pos: ChunkPos, token: u64, result: io::Result<Option<ReadChunk>> },
    Generated { pos: ChunkPos, token: u64, chunk: LevelChunk },
    StructureStarts { pos: ChunkPos, token: u64, structures: ChunkStructures },
    Wake,
}

pub struct ChunkToSave {
    pub chunk: LevelChunk,
    pub status: ChunkStatus,
    pub block_entities: Vec<Nbt>,
    pub game_time: i64,
    pub block_ticks: Vec<SavedTick<Block>>,
    pub fluid_ticks: Vec<SavedTick<Fluid>>,
    pub light: Vec<SectionLight>,
}

#[derive(Clone)]
pub struct IoReporting {
    pub reporter: ChunkIoErrorReporter,
    pub info: RegionStorageInfo,
}

enum IoRequest {
    Read { pos: ChunkPos, reply: Box<dyn FnOnce(io::Result<Option<Nbt>>) + Send> },
    ScanStructures { pos: ChunkPos, reply: Box<dyn FnOnce(io::Result<Option<Nbt>>) + Send> },
    ScanOldData { pos: ChunkPos, reply: Box<dyn FnOnce(io::Result<bool>) + Send> },
    Write { pos: ChunkPos, nbt: Nbt },
    Clear { pos: ChunkPos },
    WriteChunk(Box<ChunkToSave>),
    Flush { flush: bool, reply: Sender<io::Result<()>> },
    SetReporting(IoReporting),
}


#[derive(Clone)]
pub struct IoWorker {
    requests: Sender<IoRequest>,
    pub active_chunk_writes: Arc<AtomicUsize>,
    queued_requests: Arc<AtomicUsize>,
}

impl IoWorker {
    pub fn new(mut storage: RegionStorage) -> IoWorker {
        let (requests, incoming) = channel::<IoRequest>();
        let active_chunk_writes = Arc::new(AtomicUsize::new(0));
        let active = active_chunk_writes.clone();
        let queued_requests = Arc::new(AtomicUsize::new(0));
        let queued = queued_requests.clone();
        thread::Builder::new()
            .name(next_io_worker_name())
            .spawn(move || {
                let _frames = crate::server::main_thread_frames::enter_complete(storage_exception::store_worker_frames());
                let mut reporting: Option<IoReporting> = None;
                for request in incoming {
                    queued.fetch_sub(1, Ordering::Relaxed);
                    match request {
                        IoRequest::Read { pos, reply } => match guarded(|| storage.read(pos)) {
                            Err(e) if storage_exception::is_java_error(&e) => {
                                let thread = thread::current();
                                let name = thread.name().unwrap_or_default();
                                let id = name.rsplit('-').next().and_then(|n| n.parse::<u32>().ok()).map_or(0, |n| 111 + n);
                                crate::crash_report::on_thread_exception(name, id, &storage_exception::on_load_worker_uncaught(&e));
                                std::mem::forget(reply);
                            }
                            result => reply(result.map_err(|e| {
                                let failure = storage_exception::on_load_worker(&e);
                                warn!("Failed to read chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                                io::Error::new(e.kind(), failure)
                            })),
                        },
                        IoRequest::ScanStructures { pos, reply } => reply(guarded(|| storage.scan_structures(pos)).map_err(|e| {
                            let failure = storage_exception::on_scan_worker(&e);
                            warn!("Failed to bulk scan chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                            io::Error::new(e.kind(), failure)
                        })),
                        IoRequest::ScanOldData { pos, reply } => reply(guarded(|| storage.scan_chunk(pos)).map_err(|e| {
                            let failure = storage_exception::on_scan_worker(&e);
                            warn!("Failed to bulk scan chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                            io::Error::new(e.kind(), failure)
                        })),
                        IoRequest::Write { pos, nbt } => {
                            write(&mut storage, pos, &nbt, reporting.as_ref());
                            active.fetch_sub(1, Ordering::Relaxed);
                        }
                        IoRequest::Clear { pos } => {
                            if let Err(e) = guarded(|| storage.clear(pos)) {
                                store_failed(pos, &e, reporting.as_ref());
                            }
                        }
                        IoRequest::WriteChunk(save) => {
                            let ChunkToSave { chunk, status, block_entities, game_time, block_ticks, fluid_ticks, light } = *save;
                            match guarded(|| Ok(serialization::write(&chunk, status, block_entities, game_time, &block_ticks, &fluid_ticks, &light))) {
                                Ok(nbt) => write(&mut storage, chunk.pos, &nbt, reporting.as_ref()),
                                Err(e) => store_failed(chunk.pos, &e, reporting.as_ref()),
                            }
                            active.fetch_sub(1, Ordering::Relaxed);
                        }
                        IoRequest::Flush { flush, reply } => drop(reply.send(if flush { guarded(|| storage.flush()) } else { Ok(()) })),
                        IoRequest::SetReporting(new_reporting) => reporting = Some(new_reporting),
                    }
                }
            })
            .expect("IO worker thread");
        IoWorker { requests, active_chunk_writes, queued_requests }
    }

    fn submit(&self, request: IoRequest) -> Result<(), std::sync::mpsc::SendError<IoRequest>> {
        self.queued_requests.fetch_add(1, Ordering::Relaxed);
        let sent = self.requests.send(request);
        if sent.is_err() {
            self.queued_requests.fetch_sub(1, Ordering::Relaxed);
        }
        sent
    }

    pub fn queue_size(&self) -> usize {
        self.queued_requests.load(Ordering::Relaxed)
    }

    pub fn set_reporting(&self, reporting: IoReporting) {
        let _ = self.submit(IoRequest::SetReporting(reporting));
    }

    pub fn read(&self, pos: ChunkPos, reply: impl FnOnce(io::Result<Option<Nbt>>) + Send + 'static) {
        let _ = self.submit(IoRequest::Read { pos, reply: Box::new(reply) });
    }

    pub fn scan_structures(&self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        let (reply, result) = channel();
        let _ = self.submit(IoRequest::ScanStructures { pos, reply: Box::new(move |scanned| drop(reply.send(scanned))) });
        result.recv().map_err(|_| io::Error::other("IO worker gone"))?
    }

    pub fn scan_old_data(&self, pos: ChunkPos) -> io::Result<bool> {
        let (reply, result) = channel();
        let _ = self.submit(IoRequest::ScanOldData { pos, reply: Box::new(move |scanned| drop(reply.send(scanned))) });
        result.recv().map_err(|_| io::Error::other("IO worker gone"))?
    }

    pub fn write(&self, pos: ChunkPos, nbt: Nbt) {
        self.active_chunk_writes.fetch_add(1, Ordering::Relaxed);
        let _ = self.submit(IoRequest::Write { pos, nbt });
    }

    pub fn clear(&self, pos: ChunkPos) {
        let _ = self.submit(IoRequest::Clear { pos });
    }

    pub fn write_chunk(&self, save: ChunkToSave) {
        self.active_chunk_writes.fetch_add(1, Ordering::Relaxed);
        let _ = self.submit(IoRequest::WriteChunk(Box::new(save)));
    }

    pub fn flush(&self) -> io::Result<()> {
        self.synchronize(true)
    }

    pub fn synchronize(&self, flush: bool) -> io::Result<()> {
        let (reply, result) = channel();
        if self.submit(IoRequest::Flush { flush, reply }).is_err() {
            return Err(io::Error::other("IO worker gone"));
        }
        result.recv().unwrap_or_else(|_| Err(io::Error::other("IO worker gone")))
    }
}

fn guarded<T>(operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    crate::util::catch_throwable(operation).unwrap_or_else(|throwable| Err(io::Error::other(throwable)))
}

fn write(storage: &mut RegionStorage, pos: ChunkPos, nbt: &Nbt, reporting: Option<&IoReporting>) {
    if let Err(e) = guarded(|| storage.write(pos, nbt)) {
        store_failed(pos, &e, reporting);
    }
}

fn store_failed(pos: ChunkPos, error: &io::Error, reporting: Option<&IoReporting>) {
    let store_failure = storage_exception::on_store_worker(error);
    error!("Failed to store chunk {pos}\n{}", store_failure.print_stack_trace().trim_end());
    let Some(reporting) = reporting else { return };
    let throwable = storage_exception::completion_exception_of_store(store_failure);
    if reporting.info.type_ == "entities" {
        error!("Failed to store entity chunk {pos}\n{}", throwable.print_stack_trace().trim_end());
    }
    reporting.reporter.report_chunk_save_failure(&throwable, &reporting.info, pos);
}

#[derive(Default)]
pub struct MainThreadWakeup {
    generation: Mutex<u64>,
    condvar: Condvar,
}

impl MainThreadWakeup {
    pub fn generation(&self) -> u64 {
        *self.generation.lock().unwrap()
    }

    pub fn wake(&self) {
        *self.generation.lock().unwrap() += 1;
        self.condvar.notify_all();
    }

    pub fn wait(&self, seen: u64, timeout: std::time::Duration) {
        let generation = self.generation.lock().unwrap();
        let _ = self.condvar.wait_timeout_while(generation, timeout, |generation| *generation == seen);
    }
}

#[derive(Clone)]
pub struct ResultSender {
    sender: Sender<WorkResult>,
    pub wakeup: Arc<MainThreadWakeup>,
}

impl ResultSender {
    pub fn send(&self, result: WorkResult) -> Result<(), std::sync::mpsc::SendError<WorkResult>> {
        let sent = self.sender.send(result);
        self.wakeup.wake();
        sent
    }
}

pub fn results() -> (ResultSender, Receiver<WorkResult>) {
    let (sender, receiver) = channel();
    (ResultSender { sender, wakeup: Arc::default() }, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::java_throwable::JavaThrowable;

    #[test]
    fn a_panic_in_a_storage_operation_becomes_an_io_error() {
        let failed: io::Result<()> = guarded(|| panic!("java.lang.IllegalStateException: boom"));
        assert_eq!(JavaThrowable::from_io_error(&failed.unwrap_err()).description, "java.lang.IllegalStateException: boom");
        assert_eq!(guarded(|| Ok(5)).unwrap(), 5);
    }
}
