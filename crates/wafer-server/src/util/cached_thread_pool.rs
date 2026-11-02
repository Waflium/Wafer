use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

type Task = Box<dyn FnOnce() + Send>;

const KEEP_ALIVE: Duration = Duration::from_secs(60);

thread_local! {
    static PRIORITY: Cell<u32> = const { Cell::new(0) };
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

struct Waiter {
    task: Mutex<Option<Task>>,
    handed: Condvar,
}

struct State {
    waiting: Vec<Arc<Waiter>>,
    running: usize,
    live: Vec<(String, u32, u32)>,
}

struct Shared {
    next_name: Box<dyn Fn() -> String + Send + Sync>,
    state: Mutex<State>,
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
        let state = Mutex::new(State { waiting: Vec::new(), running: 0, live: Vec::new() });
        CachedThreadPool { shared: Arc::new(Shared { next_name: Box::new(next_name), state, finished: Condvar::new() }) }
    }

    pub fn execute(&self, task: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.state.lock().unwrap();
        state.running += 1;
        if let Some(waiter) = state.waiting.pop() {
            drop(state);
            *waiter.task.lock().unwrap() = Some(Box::new(task));
            waiter.handed.notify_one();
            return;
        }
        let name = (self.shared.next_name)();
        let priority = current_priority();
        state.live.push((name.clone(), priority, super::next_java_thread_id()));
        drop(state);
        let shared = self.shared.clone();
        let thread_name = name.clone();
        std::thread::Builder::new()
            .name(name)
            .spawn(move || {
                PRIORITY.set(priority);
                run_worker(shared, thread_name, Box::new(task))
            })
            .expect("worker thread");
    }

    pub fn await_tasks(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.state.lock().unwrap();
        while state.running > 0 {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { return };
            state = self.shared.finished.wait_timeout(state, left).unwrap().0;
        }
    }

    pub fn live_threads(&self) -> Vec<(String, u32, u32)> {
        self.shared.state.lock().unwrap().live.clone()
    }
}

fn exit(shared: &Shared, name: &str) {
    let mut state = shared.state.lock().unwrap();
    if let Some(index) = state.live.iter().position(|(live, _, _)| live == name) {
        state.live.remove(index);
    }
}

fn run(shared: &Shared, name: &str, task: Task) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task));
    let mut state = shared.state.lock().unwrap();
    state.running -= 1;
    if state.running == 0 {
        shared.finished.notify_all();
    }
    drop(state);
    if let Err(payload) = result {
        exit(shared, name);
        std::panic::resume_unwind(payload);
    }
}

fn run_worker(shared: Arc<Shared>, name: String, first: Task) {
    run(&shared, &name, first);
    let waiter = Arc::new(Waiter { task: Mutex::new(None), handed: Condvar::new() });
    loop {
        shared.state.lock().unwrap().waiting.push(waiter.clone());
        let task = waiter.task.lock().unwrap();
        let (mut task, timeout) = waiter.handed.wait_timeout_while(task, KEEP_ALIVE, |task| task.is_none()).unwrap();
        let next = match task.take() {
            Some(next) => next,
            None if timeout.timed_out() => {
                drop(task);
                let mut state = shared.state.lock().unwrap();
                match state.waiting.iter().position(|waiting| Arc::ptr_eq(waiting, &waiter)) {
                    Some(index) => {
                        state.waiting.remove(index);
                        if let Some(index) = state.live.iter().position(|(live, _, _)| *live == name) {
                            state.live.remove(index);
                        }
                        return;
                    }
                    None => {
                        drop(state);
                        let task = waiter.task.lock().unwrap();
                        let mut task = waiter.handed.wait_while(task, |task| task.is_none()).unwrap();
                        task.take().expect("a handed task")
                    }
                }
            }
            None => continue,
        };
        run(&shared, &name, next);
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
        pool.execute(second);
        while names.lock().unwrap().len() < 2 {
            std::thread::yield_now();
        }
        idle_after(&pool, release_second);
        idle_after(&pool, release_first);
        let (third, release_third) = blocking_task(&names);
        pool.execute(third);
        release_third.send(()).unwrap();
        pool.await_tasks(Duration::from_secs(5));
        assert_eq!(*names.lock().unwrap(), ["T-1", "T-2", "T-1"]);
        let live: Vec<(String, u32)> = pool.live_threads().into_iter().map(|(name, priority, _)| (name, priority)).collect();
        assert_eq!(live, [("T-1".to_string(), 5), ("T-2".to_string(), 5)]);
    }
}
