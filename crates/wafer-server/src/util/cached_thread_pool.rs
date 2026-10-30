use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

type Task = Box<dyn FnOnce() + Send>;

const KEEP_ALIVE: Duration = Duration::from_secs(60);

struct State {
    queue: VecDeque<Task>,
    idle: usize,
}

struct Shared {
    prefix: &'static str,
    worker_count: AtomicUsize,
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Clone)]
pub struct CachedThreadPool {
    shared: Arc<Shared>,
}

impl CachedThreadPool {
    pub fn new(prefix: &'static str) -> CachedThreadPool {
        CachedThreadPool { shared: Arc::new(Shared { prefix, worker_count: AtomicUsize::new(1), state: Mutex::new(State { queue: VecDeque::new(), idle: 0 }), wake: Condvar::new() }) }
    }

    pub fn execute(&self, task: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.state.lock().unwrap();
        if state.idle > state.queue.len() {
            state.queue.push_back(Box::new(task));
            self.shared.wake.notify_one();
            return;
        }
        drop(state);
        let name = format!("{}{}", self.shared.prefix, self.shared.worker_count.fetch_add(1, Ordering::Relaxed));
        let shared = self.shared.clone();
        std::thread::Builder::new().name(name).spawn(move || run_worker(shared, Box::new(task))).expect("worker thread");
    }
}

fn run_worker(shared: Arc<Shared>, first: Task) {
    first();
    loop {
        let mut state = shared.state.lock().unwrap();
        state.idle += 1;
        let (mut state, _) = shared.wake.wait_timeout_while(state, KEEP_ALIVE, |state| state.queue.is_empty()).unwrap();
        state.idle -= 1;
        match state.queue.pop_front() {
            Some(task) => {
                drop(state);
                task();
            }
            None => return,
        }
    }
}
