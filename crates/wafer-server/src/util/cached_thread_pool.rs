use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

type Task = Box<dyn FnOnce() + Send>;

const KEEP_ALIVE: Duration = Duration::from_secs(60);

struct State {
    queue: VecDeque<Task>,
    idle: usize,
    running: usize,
}

struct Shared {
    next_name: Box<dyn Fn() -> String + Send + Sync>,
    state: Mutex<State>,
    wake: Condvar,
    finished: Condvar,
}

#[derive(Clone)]
pub struct CachedThreadPool {
    shared: Arc<Shared>,
}

impl CachedThreadPool {
    pub fn new(prefix: &'static str) -> CachedThreadPool {
        let worker_count = AtomicUsize::new(1);
        CachedThreadPool::with_names(move || format!("{prefix}{}", worker_count.fetch_add(1, Ordering::Relaxed)))
    }

    pub fn with_names(next_name: impl Fn() -> String + Send + Sync + 'static) -> CachedThreadPool {
        let state = Mutex::new(State { queue: VecDeque::new(), idle: 0, running: 0 });
        CachedThreadPool { shared: Arc::new(Shared { next_name: Box::new(next_name), state, wake: Condvar::new(), finished: Condvar::new() }) }
    }

    pub fn execute(&self, task: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.state.lock().unwrap();
        state.running += 1;
        if state.idle > state.queue.len() {
            state.queue.push_back(Box::new(task));
            self.shared.wake.notify_one();
            return;
        }
        drop(state);
        let name = (self.shared.next_name)();
        let shared = self.shared.clone();
        std::thread::Builder::new().name(name).spawn(move || run_worker(shared, Box::new(task))).expect("worker thread");
    }

    pub fn await_tasks(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.state.lock().unwrap();
        while state.running > 0 {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { return };
            state = self.shared.finished.wait_timeout(state, left).unwrap().0;
        }
    }
}

fn run(shared: &Shared, task: Task) {
    task();
    let mut state = shared.state.lock().unwrap();
    state.running -= 1;
    if state.running == 0 {
        shared.finished.notify_all();
    }
}

fn run_worker(shared: Arc<Shared>, first: Task) {
    run(&shared, first);
    loop {
        let mut state = shared.state.lock().unwrap();
        state.idle += 1;
        let (mut state, _) = shared.wake.wait_timeout_while(state, KEEP_ALIVE, |state| state.queue.is_empty()).unwrap();
        state.idle -= 1;
        match state.queue.pop_front() {
            Some(task) => {
                drop(state);
                run(&shared, task);
            }
            None => return,
        }
    }
}
