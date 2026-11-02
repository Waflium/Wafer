use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::thread;

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use crate::util::java_throwable::JavaThrowable;

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

pub fn is_unregistered_holder(throwable: &JavaThrowable) -> bool {
    throwable.description.starts_with(serialization::UNREGISTERED_HOLDER_MESSAGE)
}

pub fn live_io_workers() -> Vec<(usize, u32, u32)> {
    let mut workers: Vec<(usize, u32, u32)> =
        crate::util::io_pool().live_threads().iter().filter_map(|(name, priority, id)| Some((name.rsplit('-').next()?.parse().ok()?, *priority, *id))).collect();
    workers.sort_unstable();
    workers
}

pub fn worker_main_count() -> usize {
    WORKER_NAMES.len()
}

struct WorkerPool {
    jobs: Mutex<VecDeque<(u8, Job)>>,
    available: Condvar,
    running: Vec<AtomicU8>,
}

const IDLE: u8 = u8::MAX;

static POOL: LazyLock<&'static WorkerPool> = LazyLock::new(|| {
    let threads = thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(1).clamp(1, 255);
    let running = (0..threads).map(|_| AtomicU8::new(IDLE)).collect();
    let pool: &'static WorkerPool = Box::leak(Box::new(WorkerPool { jobs: Mutex::new(VecDeque::new()), available: Condvar::new(), running }));
    for i in 0..threads {
        thread::Builder::new()
            .name(format!("Worker-Main-{}", i + 1))
            .spawn(move || {
                loop {
                    let (step, job) = {
                        let mut jobs = pool.jobs.lock().unwrap();
                        loop {
                            if let Some(job) = jobs.pop_front() {
                                break job;
                            }
                            jobs = pool.available.wait(jobs).unwrap();
                        }
                    };
                    pool.running[i].store(step, Ordering::Relaxed);
                    let result = crate::util::catch_exception(job);
                    pool.running[i].store(IDLE, Ordering::Relaxed);
                    if let Err(exception) = result {
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
    submit(IDLE, Box::new(job));
}

pub fn execute_step(status: ChunkStatus, job: impl FnOnce() + Send + 'static) {
    submit(status.index() as u8, Box::new(job));
}

fn submit(step: u8, job: Job) {
    let pool = *POOL;
    pool.jobs.lock().unwrap().push_back((step, job));
    pool.available.notify_one();
}

pub fn running_worker_steps() -> Vec<(usize, ChunkStatus)> {
    let Some(pool) = LazyLock::get(&POOL) else { return Vec::new() };
    pool.running
        .iter()
        .enumerate()
        .filter_map(|(index, status)| ChunkStatus::ALL.get(status.load(Ordering::Relaxed) as usize).map(|&status| (index + 1, status)))
        .collect()
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
    Write { pos: ChunkPos, nbt: Result<Nbt, JavaThrowable> },
    Clear { pos: ChunkPos },
    WriteChunk(Box<ChunkToSave>),
    Synchronize { flush: bool, reply: Sender<io::Result<()>> },
    FinishSynchronize { flush: bool, reply: Sender<io::Result<()>> },
}

enum IoTask {
    Foreground(IoRequest),
    StorePendingChunk,
}

struct IoQueue {
    foreground: VecDeque<IoRequest>,
    background: usize,
    running: bool,
}

impl IoQueue {
    fn pop(&mut self) -> Option<IoTask> {
        match self.foreground.pop_front() {
            Some(request) => Some(IoTask::Foreground(request)),
            None if self.background > 0 => {
                self.background -= 1;
                Some(IoTask::StorePendingChunk)
            }
            None => None,
        }
    }

    fn is_empty(&self) -> bool {
        self.foreground.is_empty() && self.background == 0
    }
}

struct PendingStore {
    data: Option<Nbt>,
    sequence: u64,
    active_writes: usize,
}

struct PendingSynchronize {
    before: u64,
    flush: bool,
    reply: Sender<io::Result<()>>,
}

#[derive(Default)]
struct PendingWrites {
    stores: crate::util::FastIndexMap<ChunkPos, PendingStore>,
    next_sequence: u64,
    synchronizing: Vec<PendingSynchronize>,
}

struct IoShared {
    queue: Mutex<IoQueue>,
    storage: Mutex<RegionStorage>,
    pending_writes: Mutex<PendingWrites>,
    reporting: Mutex<Option<IoReporting>>,
    active_chunk_writes: Arc<AtomicUsize>,
    queued_requests: Arc<AtomicUsize>,
}

impl IoShared {
    fn schedule(self: &Arc<IoShared>, request: IoRequest) {
        self.queued_requests.fetch_add(1, Ordering::Relaxed);
        let mut queue = self.queue.lock().unwrap();
        queue.foreground.push_back(request);
        register_for_execution(self, &mut queue);
    }

    fn tell_store_pending(self: &Arc<IoShared>) {
        self.queued_requests.fetch_add(1, Ordering::Relaxed);
        let mut queue = self.queue.lock().unwrap();
        queue.background += 1;
        register_for_execution(self, &mut queue);
    }
}

fn register_for_execution(shared: &Arc<IoShared>, queue: &mut IoQueue) {
    if !queue.running && !queue.is_empty() {
        queue.running = true;
        let shared = shared.clone();
        crate::util::io_pool().execute(move || run_io_worker(shared));
    }
}

fn run_io_worker(shared: Arc<IoShared>) {
    let task = shared.queue.lock().unwrap().pop();
    let thrown = task.and_then(|task| {
        shared.queued_requests.fetch_sub(1, Ordering::Relaxed);
        let _frames = crate::server::main_thread_frames::enter_complete(storage_exception::store_worker_frames());
        match task {
            IoTask::Foreground(request) => {
                let thrown = process_io_request(&shared, request);
                if thrown.is_none() {
                    shared.tell_store_pending();
                }
                thrown
            }
            IoTask::StorePendingChunk => {
                store_pending_chunk(&shared);
                None
            }
        }
    });
    {
        let mut queue = shared.queue.lock().unwrap();
        queue.running = false;
        register_for_execution(&shared, &mut queue);
    }
    if let Some(throwable) = thrown {
        crate::crash_report::throw_java_exception(throwable);
    }
}

fn store_pending_chunk(shared: &Arc<IoShared>) {
    let Some((pos, pending)) = shared.pending_writes.lock().unwrap().stores.shift_remove_index(0) else { return };
    {
        let mut storage = shared.storage.lock().unwrap();
        let reporting = shared.reporting.lock().unwrap();
        match &pending.data {
            Some(nbt) => write(&mut storage, pos, nbt, reporting.as_ref()),
            None => {
                if let Err(e) = guarded(|| storage.clear(pos)) {
                    store_failed(pos, &e, reporting.as_ref());
                }
            }
        }
    }
    shared.active_chunk_writes.fetch_sub(pending.active_writes, Ordering::Relaxed);
    finish_synchronizations(shared);
    shared.tell_store_pending();
}

fn store(shared: &IoShared, pos: ChunkPos, data: Option<Nbt>, active_writes: usize) {
    let mut pending_writes = shared.pending_writes.lock().unwrap();
    let sequence = pending_writes.next_sequence;
    match pending_writes.stores.get_mut(&pos) {
        Some(pending) => {
            pending.data = data;
            pending.active_writes += active_writes;
        }
        None => {
            pending_writes.next_sequence += 1;
            pending_writes.stores.insert(pos, PendingStore { data, sequence, active_writes });
        }
    }
}

fn finish_synchronizations(shared: &Arc<IoShared>) {
    let finished: Vec<PendingSynchronize> = {
        let mut pending_writes = shared.pending_writes.lock().unwrap();
        let first_pending = pending_writes.stores.first().map_or(u64::MAX, |(_, pending)| pending.sequence);
        let (finished, waiting) = std::mem::take(&mut pending_writes.synchronizing).into_iter().partition(|synchronize| first_pending >= synchronize.before);
        pending_writes.synchronizing = waiting;
        finished
    };
    for synchronize in finished {
        shared.schedule(IoRequest::FinishSynchronize { flush: synchronize.flush, reply: synchronize.reply });
    }
}

fn process_io_request(shared: &Arc<IoShared>, request: IoRequest) -> Option<JavaThrowable> {
    let active = &shared.active_chunk_writes;
    match request {
        IoRequest::Write { pos, nbt } => {
            match nbt {
                Ok(nbt) => store(shared, pos, Some(nbt), 1),
                Err(throwable) => return Some(throwable),
            }
            return None;
        }
        IoRequest::Clear { pos } => {
            store(shared, pos, None, 0);
            return None;
        }
        IoRequest::WriteChunk(save) => {
            let ChunkToSave { chunk, status, block_entities, game_time, block_ticks, fluid_ticks, light } = *save;
            match guarded(|| Ok(serialization::write(&chunk, status, block_entities, game_time, &block_ticks, &fluid_ticks, &light))) {
                Ok(nbt) => store(shared, chunk.pos, Some(nbt), 1),
                Err(e) if is_unregistered_holder(&JavaThrowable::from_io_error(&e)) => return Some(JavaThrowable::from_io_error(&e)),
                Err(e) => {
                    store_failed(chunk.pos, &e, shared.reporting.lock().unwrap().as_ref());
                    active.fetch_sub(1, Ordering::Relaxed);
                }
            }
            return None;
        }
        IoRequest::Synchronize { flush, reply } => {
            {
                let mut pending_writes = shared.pending_writes.lock().unwrap();
                let before = pending_writes.next_sequence;
                pending_writes.synchronizing.push(PendingSynchronize { before, flush, reply });
            }
            finish_synchronizations(shared);
            return None;
        }
        _ => {}
    }
    let pending_data = |pos: ChunkPos| shared.pending_writes.lock().unwrap().stores.get(&pos).map(|pending| pending.data.clone());
    let mut storage = shared.storage.lock().unwrap();
    let storage = &mut *storage;
    match request {
        IoRequest::Read { pos, reply } => match pending_data(pos) {
            Some(data) => reply(Ok(data)),
            None => match guarded(|| storage.read(pos)) {
                Err(e) if storage_exception::is_java_error(&e) => {
                    std::mem::forget(reply);
                    return Some(storage_exception::on_load_worker_uncaught(&e));
                }
                result => reply(result.map_err(|e| {
                    let mut failure = storage_exception::on_load_worker(&e);
                    crate::world::level::chunk::storage::chunk_stream::create_reported_crash_report(&mut failure);
                    warn!("Failed to read chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                    io::Error::new(e.kind(), failure)
                })),
            },
        },
        IoRequest::ScanStructures { pos, reply } => match pending_data(pos) {
            Some(data) => reply(Ok(data)),
            None => reply(guarded(|| storage.scan_structures(pos)).map_err(|e| {
                let failure = storage_exception::on_scan_worker(&e);
                warn!("Failed to bulk scan chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                io::Error::new(e.kind(), failure)
            })),
        },
        IoRequest::ScanOldData { pos, reply } => match pending_data(pos) {
            Some(data) => reply(Ok(data.as_ref().is_some_and(crate::world::level::chunk::storage::is_old_chunk))),
            None => reply(guarded(|| storage.scan_chunk(pos)).map_err(|e| {
                let failure = storage_exception::on_scan_worker(&e);
                warn!("Failed to bulk scan chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                io::Error::new(e.kind(), failure)
            })),
        },
        IoRequest::FinishSynchronize { flush, reply } => drop(reply.send(if flush { guarded(|| storage.flush()) } else { Ok(()) })),
        IoRequest::Write { .. } | IoRequest::Clear { .. } | IoRequest::WriteChunk(_) | IoRequest::Synchronize { .. } => {}
    }
    None
}

#[derive(Clone)]
pub struct IoWorker {
    shared: Arc<IoShared>,
    pub active_chunk_writes: Arc<AtomicUsize>,
    queued_requests: Arc<AtomicUsize>,
}

impl IoWorker {
    pub fn new(storage: RegionStorage) -> IoWorker {
        let active_chunk_writes = Arc::new(AtomicUsize::new(0));
        let queued_requests = Arc::new(AtomicUsize::new(0));
        let shared = Arc::new(IoShared {
            queue: Mutex::new(IoQueue { foreground: VecDeque::new(), background: 0, running: false }),
            storage: Mutex::new(storage),
            pending_writes: Mutex::new(PendingWrites::default()),
            reporting: Mutex::new(None),
            active_chunk_writes: active_chunk_writes.clone(),
            queued_requests: queued_requests.clone(),
        });
        IoWorker { shared, active_chunk_writes, queued_requests }
    }

    fn submit(&self, request: IoRequest) -> Result<(), std::sync::mpsc::SendError<IoRequest>> {
        self.shared.schedule(request);
        Ok(())
    }

    pub fn queue_size(&self) -> usize {
        self.queued_requests.load(Ordering::Relaxed)
    }

    pub fn set_reporting(&self, reporting: IoReporting) {
        *self.shared.reporting.lock().unwrap() = Some(reporting);
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
        self.store(pos, Ok(nbt));
    }

    pub fn store(&self, pos: ChunkPos, nbt: Result<Nbt, JavaThrowable>) {
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
        if self.submit(IoRequest::Synchronize { flush, reply }).is_err() {
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
