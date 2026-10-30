use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicI64, AtomicU8, AtomicU64, Ordering};

const SLOTS: usize = 2048;
const FREE: u8 = 0;
const CLAIMED: u8 = 1;
const LIVE: u8 = 2;

struct Slot {
    state: AtomicU8,
    allocated: AtomicU64,
    freed: AtomicU64,
    os_thread_id: AtomicI64,
}

#[allow(clippy::declare_interior_mutable_const)]
const EMPTY_SLOT: Slot = Slot { state: AtomicU8::new(FREE), allocated: AtomicU64::new(0), freed: AtomicU64::new(0), os_thread_id: AtomicI64::new(0) };

static TABLE: [Slot; SLOTS] = [EMPTY_SLOT; SLOTS];
static DEAD_ALLOCATED: AtomicU64 = AtomicU64::new(0);
static DEAD_FREED: AtomicU64 = AtomicU64::new(0);
static UNTRACKED_ALLOCATED: AtomicU64 = AtomicU64::new(0);
static UNTRACKED_FREED: AtomicU64 = AtomicU64::new(0);

struct Release;

impl Drop for Release {
    fn drop(&mut self) {
        let slot = SLOT.with(|slot| slot.replace(u32::MAX - 1));
        if let Some(slot) = TABLE.get(slot as usize) {
            DEAD_ALLOCATED.fetch_add(slot.allocated.swap(0, Ordering::Relaxed), Ordering::Relaxed);
            DEAD_FREED.fetch_add(slot.freed.swap(0, Ordering::Relaxed), Ordering::Relaxed);
            slot.state.store(FREE, Ordering::Release);
        }
    }
}

thread_local! {
    static SLOT: Cell<u32> = const { Cell::new(u32::MAX) };
    static REGISTERING: Cell<bool> = const { Cell::new(false) };
    static RELEASE: Release = const { Release };
}

pub struct CountingAllocator;

fn current_slot() -> Option<&'static Slot> {
    let slot = SLOT.try_with(Cell::get).ok()?;
    if slot == u32::MAX {
        return register();
    }
    TABLE.get(slot as usize)
}

fn register() -> Option<&'static Slot> {
    if REGISTERING.try_with(|registering| registering.replace(true)).ok()? {
        return None;
    }
    let claimed = TABLE.iter().position(|slot| slot.state.compare_exchange(FREE, CLAIMED, Ordering::Acquire, Ordering::Relaxed).is_ok());
    if let Some(index) = claimed {
        let slot = &TABLE[index];
        slot.allocated.store(0, Ordering::Relaxed);
        slot.freed.store(0, Ordering::Relaxed);
        slot.os_thread_id.store(super::thread::os_thread_id(), Ordering::Relaxed);
        let _ = SLOT.try_with(|current| current.set(index as u32));
        let _ = RELEASE.try_with(|_| ());
        slot.state.store(LIVE, Ordering::Release);
    }
    let _ = REGISTERING.try_with(|registering| registering.set(false));
    claimed.map(|index| &TABLE[index])
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record_free(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_free(layout.size());
        record_allocation(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn record_allocation(size: usize) {
    match current_slot() {
        Some(slot) => slot.allocated.store(slot.allocated.load(Ordering::Relaxed) + size as u64, Ordering::Relaxed),
        None => {
            UNTRACKED_ALLOCATED.fetch_add(size as u64, Ordering::Relaxed);
        }
    }
}

fn record_free(size: usize) {
    match current_slot() {
        Some(slot) => slot.freed.store(slot.freed.load(Ordering::Relaxed) + size as u64, Ordering::Relaxed),
        None => {
            UNTRACKED_FREED.fetch_add(size as u64, Ordering::Relaxed);
        }
    }
}

pub struct ThreadAllocation {
    pub os_thread_id: i64,
    pub allocated: u64,
}

pub fn live_threads() -> Vec<ThreadAllocation> {
    TABLE
        .iter()
        .filter(|slot| slot.state.load(Ordering::Acquire) == LIVE)
        .map(|slot| ThreadAllocation { os_thread_id: slot.os_thread_id.load(Ordering::Relaxed), allocated: slot.allocated.load(Ordering::Relaxed) })
        .collect()
}

pub fn total_allocated() -> u64 {
    TABLE.iter().map(|slot| slot.allocated.load(Ordering::Relaxed)).sum::<u64>() + DEAD_ALLOCATED.load(Ordering::Relaxed) + UNTRACKED_ALLOCATED.load(Ordering::Relaxed)
}

pub fn heap_used() -> u64 {
    let freed = TABLE.iter().map(|slot| slot.freed.load(Ordering::Relaxed)).sum::<u64>() + DEAD_FREED.load(Ordering::Relaxed) + UNTRACKED_FREED.load(Ordering::Relaxed);
    total_allocated().saturating_sub(freed)
}
