use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

type Task = Box<dyn FnOnce() + Send>;

const KEEP_ALIVE: Duration = Duration::from_secs(60);

thread_local! {
    static PRIORITY: Cell<u32> = const { Cell::new(0) };
    static JAVA_THREAD_ID: Cell<u32> = const { Cell::new(0) };
}

pub fn current_java_thread_id() -> u32 {
    JAVA_THREAD_ID.get()
}

pub struct PriorityScope(u32);

impl Drop for PriorityScope {
    fn drop(&mut self) {
        PRIORITY.set(self.0);
    }
}

pub fn enter_priority(priority: u32) -> PriorityScope {
    PriorityScope(PRIORITY.replace(priority))
}

fn current_priority() -> u32 {
    match PRIORITY.get() {
        0 => match std::thread::current().name() {
            Some("Server thread" | "Server Watchdog" | "Server console handler") => 8,
            _ => 5,
        },
        priority => priority,
    }
}

enum Slot {
    Empty,
    Task(Task),
    Interrupted,
}

struct Waiter {
    slot: Mutex<Slot>,
    handed: Condvar,
}

struct State {
    waiting: Vec<Arc<Waiter>>,
    workers: usize,
    shutdown: bool,
    live: Vec<(String, u32, u32)>,
}

struct Shared {
    next_name: Box<dyn Fn() -> String + Send + Sync>,
    state: Mutex<State>,
    changed: Condvar,
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
        let state = Mutex::new(State { waiting: Vec::new(), workers: 0, shutdown: false, live: Vec::new() });
        CachedThreadPool { shared: Arc::new(Shared { next_name: Box::new(next_name), state, changed: Condvar::new() }) }
    }

    pub fn execute(&self, task: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.state.lock().unwrap();
        if let Some(waiter) = state.waiting.pop() {
            drop(state);
            *waiter.slot.lock().unwrap() = Slot::Task(Box::new(task));
            waiter.handed.notify_one();
            return;
        }
        add_worker(&self.shared, &mut state, Some(Box::new(task)));
    }

    pub fn shutdown_and_await(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.state.lock().unwrap();
        state.shutdown = true;
        for waiter in std::mem::take(&mut state.waiting) {
            *waiter.slot.lock().unwrap() = Slot::Interrupted;
            waiter.handed.notify_one();
        }
        while state.workers > 0 {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { return };
            state = self.shared.changed.wait_timeout(state, left).unwrap().0;
        }
    }

    pub fn await_exit(&self) {
        let mut state = self.shared.state.lock().unwrap();
        while !state.live.is_empty() {
            state = self.shared.changed.wait(state).unwrap();
        }
    }

    pub fn live_threads(&self) -> Vec<(String, u32, u32)> {
        self.shared.state.lock().unwrap().live.clone()
    }
}

fn add_worker(shared: &Arc<Shared>, state: &mut MutexGuard<State>, first: Option<Task>) {
    let name = (shared.next_name)();
    let priority = current_priority();
    let java_thread_id = super::next_java_thread_id();
    state.workers += 1;
    state.live.push((name.clone(), priority, java_thread_id));
    let shared = shared.clone();
    let thread_name = name.clone();
    std::thread::Builder::new()
        .name(name)
        .spawn(move || {
            PRIORITY.set(priority);
            JAVA_THREAD_ID.set(java_thread_id);
            run_worker(shared, thread_name, first)
        })
        .expect("worker thread");
}

fn process_worker_exit(shared: &Shared, state: &mut State) {
    state.workers -= 1;
    shared.changed.notify_all();
}

fn exit(shared: &Shared, name: &str) {
    let mut state = shared.state.lock().unwrap();
    if let Some(index) = state.live.iter().position(|(live, _, _)| live == name) {
        state.live.remove(index);
    }
    shared.changed.notify_all();
}

fn run_worker(shared: Arc<Shared>, name: String, first: Option<Task>) {
    let waiter = Arc::new(Waiter { slot: Mutex::new(Slot::Empty), handed: Condvar::new() });
    let mut task = first;
    loop {
        let current = match task.take() {
            Some(current) => current,
            None => match get_task(&shared, &waiter) {
                Some(next) => next,
                None => break,
            },
        };
        if let Err(throwable) = crate::util::catch_throwable(current) {
            {
                let mut state = shared.state.lock().unwrap();
                process_worker_exit(&shared, &mut state);
                if !state.shutdown {
                    add_worker(&shared, &mut state, None);
                }
            }
            crate::crash_report::on_thread_exception(&name, JAVA_THREAD_ID.get(), PRIORITY.get(), &throwable);
            exit(&shared, &name);
            return;
        }
    }
    process_worker_exit(&shared, &mut shared.state.lock().unwrap());
    exit(&shared, &name);
}

fn get_task(shared: &Shared, waiter: &Arc<Waiter>) -> Option<Task> {
    {
        let mut state = shared.state.lock().unwrap();
        if state.shutdown {
            return None;
        }
        state.waiting.push(waiter.clone());
    }
    let slot = waiter.slot.lock().unwrap();
    let (mut slot, timeout) = waiter.handed.wait_timeout_while(slot, KEEP_ALIVE, |slot| matches!(slot, Slot::Empty)).unwrap();
    match std::mem::replace(&mut *slot, Slot::Empty) {
        Slot::Task(next) => return Some(next),
        Slot::Interrupted => return None,
        Slot::Empty => debug_assert!(timeout.timed_out()),
    }
    drop(slot);
    let mut state = shared.state.lock().unwrap();
    match state.waiting.iter().position(|waiting| Arc::ptr_eq(waiting, waiter)) {
        Some(index) => {
            state.waiting.remove(index);
            None
        }
        None => {
            drop(state);
            let slot = waiter.slot.lock().unwrap();
            let mut slot = waiter.handed.wait_while(slot, |slot| matches!(slot, Slot::Empty)).unwrap();
            match std::mem::replace(&mut *slot, Slot::Empty) {
                Slot::Task(next) => Some(next),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocking_task(names: &Arc<Mutex<Vec<String>>>) -> (impl FnOnce() + Send + 'static, std::sync::mpsc::Sender<()>) {
        let names = names.clone();
        let (release, gate) = std::sync::mpsc::channel::<()>();
        let task = move || {
            names.lock().unwrap().push(std::thread::current().name().unwrap().to_string());
            let _ = gate.recv();
        };
        (task, release)
    }

    fn idle_after(pool: &CachedThreadPool, release: std::sync::mpsc::Sender<()>) {
        let before = pool.shared.state.lock().unwrap().waiting.len();
        release.send(()).unwrap();
        while pool.shared.state.lock().unwrap().waiting.len() == before {
            std::thread::yield_now();
        }
    }

    #[test]
    fn idle_threads_take_new_tasks_most_recent_first() {
        let pool = CachedThreadPool::new("T-");
        let names = Arc::new(Mutex::new(Vec::new()));
        let (first, release_first) = blocking_task(&names);
        let (second, release_second) = blocking_task(&names);
        pool.execute(first);
        while names.lock().unwrap().is_empty() {
            std::thread::yield_now();
        }
        pool.execute(second);
        while names.lock().unwrap().len() < 2 {
            std::thread::yield_now();
        }
        idle_after(&pool, release_second);
        idle_after(&pool, release_first);
        let (third, release_third) = blocking_task(&names);
        pool.execute(third);
        idle_after(&pool, release_third);
        assert_eq!(*names.lock().unwrap(), ["T-1", "T-2", "T-1"]);
        let live: Vec<(String, u32)> = pool.live_threads().into_iter().map(|(name, priority, _)| (name, priority)).collect();
        assert_eq!(live, [("T-1".to_string(), 5), ("T-2".to_string(), 5)]);
        pool.shutdown_and_await(Duration::from_secs(5));
        pool.await_exit();
        assert!(pool.live_threads().is_empty());
    }
}
