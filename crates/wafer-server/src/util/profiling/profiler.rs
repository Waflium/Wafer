use std::cell::RefCell;
use std::sync::atomic::{AtomicI32, Ordering};

use super::active_profiler::{SharedProfiler, lock};
use super::metrics::metric_category::MetricCategory;

static ACTIVE_COUNT: AtomicI32 = AtomicI32::new(0);

thread_local! {
    static ACTIVE: RefCell<Option<Vec<SharedProfiler>>> = const { RefCell::new(None) };
}

pub struct Scope {
    active: bool,
}

pub fn use_fillers(fillers: Vec<SharedProfiler>) -> Scope {
    if fillers.is_empty() {
        return Scope { active: false };
    }
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        assert!(active.is_none(), "Profiler is already active");
        for filler in &fillers {
            lock(filler).start_tick();
        }
        *active = Some(fillers);
    });
    ACTIVE_COUNT.fetch_add(1, Ordering::Relaxed);
    Scope { active: true }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let fillers = ACTIVE.with(|active| active.borrow_mut().take());
        ACTIVE_COUNT.fetch_sub(1, Ordering::Relaxed);
        for filler in fillers.iter().flatten() {
            lock(filler).end_tick();
        }
    }
}

#[inline]
pub fn is_active() -> bool {
    ACTIVE_COUNT.load(Ordering::Relaxed) != 0
}

fn with_active(action: impl Fn(&mut super::active_profiler::ActiveProfiler)) {
    ACTIVE.with(|active| {
        if let Some(fillers) = active.borrow().as_ref() {
            for filler in fillers {
                action(&mut lock(filler));
            }
        }
    });
}

#[inline]
pub fn push(name: &str) {
    if is_active() {
        with_active(|profiler| profiler.push(name));
    }
}

#[inline]
pub fn push_with(name: impl FnOnce() -> String) {
    if is_active() {
        let name = name();
        with_active(|profiler| profiler.push(&name));
    }
}

#[inline]
pub fn pop() {
    if is_active() {
        with_active(|profiler| profiler.pop());
    }
}

#[inline]
pub fn pop_push(name: &str) {
    if is_active() {
        with_active(|profiler| profiler.pop_push(name));
    }
}

#[inline]
pub fn pop_push_with(name: impl FnOnce() -> String) {
    if is_active() {
        let name = name();
        with_active(|profiler| profiler.pop_push(&name));
    }
}

#[inline]
pub fn increment_counter(name: &str, amount: i32) {
    if is_active() {
        with_active(|profiler| profiler.increment_counter(name, amount));
    }
}

#[inline]
pub fn mark_for_charting(category: MetricCategory) {
    if is_active() {
        with_active(|profiler| profiler.mark_for_charting(category));
    }
}

pub struct Zone {
    active: bool,
}

#[inline]
pub fn zone(name: &str) -> Zone {
    push(name);
    Zone { active: is_active() }
}

#[inline]
pub fn zone_with(name: impl FnOnce() -> String) -> Zone {
    push_with(name);
    Zone { active: is_active() }
}

impl Drop for Zone {
    #[inline]
    fn drop(&mut self) {
        if self.active {
            pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::util::profiling::active_profiler::ActiveProfiler;

    #[test]
    fn zones_nest_under_root_and_count_every_pop() {
        let profiler = Arc::new(Mutex::new(ActiveProfiler::new(Box::new(|| 0), true)));
        {
            let _scope = use_fillers(vec![profiler.clone()]);
            push("a");
            {
                let _zone = zone("b");
                increment_counter("items", 3);
            }
            pop_push_with(|| "c".to_string());
            pop();
        }
        let profiler = lock(&profiler);
        assert_eq!(profiler.get_entry("root").map(|entry| entry.count), Some(1));
        assert_eq!(profiler.get_entry("root\u{1e}a").map(|entry| entry.count), Some(1));
        assert_eq!(profiler.get_entry("root\u{1e}a\u{1e}b").map(|entry| entry.counters.iter().map(|(_, value)| *value).sum::<i64>()), Some(3));
        assert_eq!(profiler.get_entry("root\u{1e}c").map(|entry| entry.count), Some(1));
    }

    #[test]
    fn nothing_is_recorded_without_a_scope() {
        push("outside");
        pop();
        assert!(!is_active() || ACTIVE.with(|active| active.borrow().is_none()));
    }
}
