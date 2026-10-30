use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::chunk::{self, HEADER_SIZE, Header};
use super::event_type::{EventType, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordingState {
    New,
    Running,
    Stopped,
}

pub struct Recording {
    id: i64,
    name: Mutex<String>,
    destination: Mutex<Option<PathBuf>>,
    state: Mutex<RecordingState>,
    start_ticks: AtomicI64,
    start_nanos: AtomicI64,
    events: Mutex<Vec<u8>>,
}

struct PeriodicEvent {
    event_type: &'static EventType,
    period: Duration,
    task: Arc<dyn Fn() + Send + Sync>,
    next: Option<Instant>,
}

static NEXT_ID: AtomicI64 = AtomicI64::new(1);
static EVENT_TYPES: Mutex<Vec<&'static EventType>> = Mutex::new(Vec::new());
static ACTIVE: Mutex<Option<Arc<Recording>>> = Mutex::new(None);
static PERIODIC: Mutex<Vec<PeriodicEvent>> = Mutex::new(Vec::new());

pub fn register(event_type: &'static EventType) {
    let mut types = EVENT_TYPES.lock().unwrap();
    if !types.iter().any(|registered| std::ptr::eq(*registered, event_type)) {
        types.push(event_type);
    }
}

pub fn add_periodic_event(event_type: &'static EventType, period: Duration, task: impl Fn() + Send + Sync + 'static) {
    let mut periodic = PERIODIC.lock().unwrap();
    periodic.retain(|existing| !std::ptr::eq(existing.event_type, event_type));
    periodic.push(PeriodicEvent { event_type, period, task: Arc::new(task), next: None });
}

pub fn epoch_nanos() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_nanos() as i64)
}

pub(super) fn append(event_type: &EventType, start_ticks: i64, end_ticks: i64, values: &[Value]) {
    let thread = if event_type.has_thread { super::thread::current() } else { 0 };
    let bytes = chunk::event(event_type, start_ticks, end_ticks, thread, values);
    let active = ACTIVE.lock().unwrap().clone();
    if let Some(recording) = active {
        recording.events.lock().unwrap().extend_from_slice(&bytes);
    }
}

impl Recording {
    pub fn new() -> Arc<Recording> {
        Arc::new(Recording {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            name: Mutex::new(String::new()),
            destination: Mutex::new(None),
            state: Mutex::new(RecordingState::New),
            start_ticks: AtomicI64::new(0),
            start_nanos: AtomicI64::new(0),
            events: Mutex::new(Vec::new()),
        })
    }

    pub fn get_id(&self) -> i64 {
        self.id
    }

    pub fn get_name(&self) -> String {
        self.name.lock().unwrap().clone()
    }

    pub fn set_name(&self, name: String) {
        *self.name.lock().unwrap() = name;
    }

    pub fn get_destination(&self) -> Option<PathBuf> {
        self.destination.lock().unwrap().clone()
    }

    pub fn set_destination(&self, destination: PathBuf) {
        *self.destination.lock().unwrap() = Some(destination);
    }

    pub fn get_state(&self) -> RecordingState {
        *self.state.lock().unwrap()
    }

    pub fn start(self: &Arc<Recording>) {
        self.start_ticks.store(super::ticks(), Ordering::Relaxed);
        self.start_nanos.store(epoch_nanos(), Ordering::Relaxed);
        *self.state.lock().unwrap() = RecordingState::Running;
        *ACTIVE.lock().unwrap() = Some(self.clone());
        for event_type in EVENT_TYPES.lock().unwrap().iter() {
            event_type.set_enabled(true);
        }
        let now = Instant::now();
        for periodic in PERIODIC.lock().unwrap().iter_mut() {
            periodic.next = Some(now + periodic.period);
        }
        let recording = self.clone();
        let _ = std::thread::Builder::new().name("RFR Periodic Tasks".into()).spawn(move || run_periodic_tasks(&recording));
    }

    pub fn stop(&self) -> std::io::Result<()> {
        for event_type in EVENT_TYPES.lock().unwrap().iter() {
            event_type.set_enabled(false);
        }
        {
            let mut active = ACTIVE.lock().unwrap();
            if active.as_ref().is_some_and(|recording| recording.id == self.id) {
                *active = None;
            }
        }
        *self.state.lock().unwrap() = RecordingState::Stopped;
        match self.get_destination() {
            Some(destination) => self.write(&destination),
            None => Ok(()),
        }
    }

    fn write(&self, destination: &Path) -> std::io::Result<()> {
        let start_ticks = self.start_ticks.load(Ordering::Relaxed);
        let start_nanos = self.start_nanos.load(Ordering::Relaxed);
        let end_ticks = super::ticks();
        let types: Vec<&'static EventType> = EVENT_TYPES.lock().unwrap().clone();
        let metadata = chunk::metadata(start_ticks, &types, gmt_offset_millis());
        let events = std::mem::take(&mut *self.events.lock().unwrap());
        let checkpoint = chunk::checkpoint(end_ticks, &super::thread::registered());
        let constant_pool_offset = HEADER_SIZE + metadata.len() + events.len();
        let header = Header {
            chunk_size: (constant_pool_offset + checkpoint.len()) as i64,
            constant_pool_offset: constant_pool_offset as i64,
            metadata_offset: HEADER_SIZE as i64,
            start_nanos,
            duration_nanos: epoch_nanos() - start_nanos,
            start_ticks,
            ticks_per_second: super::TICKS_PER_SECOND,
        };
        let mut file = std::io::BufWriter::new(std::fs::File::create(destination)?);
        file.write_all(&header.write())?;
        file.write_all(&metadata)?;
        file.write_all(&events)?;
        file.write_all(&checkpoint)?;
        file.flush()
    }
}

fn gmt_offset_millis() -> i64 {
    chrono::Local::now().offset().local_minus_utc() as i64 * 1000
}

fn run_periodic_tasks(recording: &Arc<Recording>) {
    while recording.get_state() == RecordingState::Running {
        let now = Instant::now();
        let due: Vec<Arc<dyn Fn() + Send + Sync>> = {
            let mut periodic = PERIODIC.lock().unwrap();
            periodic
                .iter_mut()
                .filter(|periodic| periodic.next.is_some_and(|next| next <= now))
                .map(|periodic| {
                    periodic.next = Some(now + periodic.period);
                    periodic.task.clone()
                })
                .collect()
        };
        for task in due {
            task();
        }
        let next = PERIODIC.lock().unwrap().iter().filter_map(|periodic| periodic.next).min();
        let wait = next.map_or(Duration::from_millis(100), |next| next.saturating_duration_since(Instant::now())).min(Duration::from_millis(100));
        std::thread::sleep(wait);
    }
}
