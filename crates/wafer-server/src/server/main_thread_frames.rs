use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};

use super::command_frames::{PREPARE_LEVELS, SPIN, TICK_LOOP};

const CAPACITY: usize = 512;
const ABSOLUTE: u64 = 1;
const LEN_SHIFT: u32 = 48;
const ADDRESS_MASK: u64 = (1 << LEN_SHIFT) - 1;

struct Stack {
    scopes: [AtomicU64; CAPACITY],
    len: AtomicUsize,
}

impl Stack {
    const fn new() -> Stack {
        Stack { scopes: [const { AtomicU64::new(0) }; CAPACITY], len: AtomicUsize::new(0) }
    }

    fn push(&self, frames: &'static [&'static str], absolute: bool) {
        let len = self.len.load(Ordering::Relaxed);
        if len < CAPACITY {
            self.scopes[len].store(encode(frames, absolute), Ordering::Relaxed);
        }
        self.len.store(len + 1, Ordering::Release);
    }

    fn pop(&self) {
        let len = self.len.load(Ordering::Relaxed);
        self.len.store(len.saturating_sub(1), Ordering::Release);
    }

    fn replace_top(&self, frames: &'static [&'static str]) {
        let len = self.len.load(Ordering::Relaxed);
        if let Some(top) = len.checked_sub(1).and_then(|top| self.scopes.get(top)) {
            let (_, absolute) = decode(top.load(Ordering::Relaxed));
            top.store(encode(frames, absolute), Ordering::Release);
        }
    }

    fn frames(&self) -> Vec<&'static str> {
        let len = self.len.load(Ordering::Acquire).min(CAPACITY);
        let mut frames = Vec::new();
        for scope in self.scopes[..len].iter().rev() {
            let (scope_frames, absolute) = decode(scope.load(Ordering::Relaxed));
            frames.extend_from_slice(scope_frames);
            if absolute {
                return frames;
            }
        }
        frames.extend(SPIN);
        frames
    }
}

fn encode(frames: &'static [&'static str], absolute: bool) -> u64 {
    let address = frames.as_ptr() as u64;
    if address > ADDRESS_MASK || frames.len() >= 1 << (64 - LEN_SHIFT) {
        return encode(&[], absolute);
    }
    address | ((frames.len() as u64) << LEN_SHIFT) | if absolute { ABSOLUTE } else { 0 }
}

fn decode(scope: u64) -> (&'static [&'static str], bool) {
    let len = (scope >> LEN_SHIFT) as usize;
    if len == 0 {
        return (&[], scope & ABSOLUTE != 0);
    }
    let address = (scope & ADDRESS_MASK & !ABSOLUTE) as *const &'static str;
    (unsafe { std::slice::from_raw_parts(address, len) }, scope & ABSOLUTE != 0)
}

thread_local! {
    static LOCAL: Stack = const { Stack::new() };
    static PUBLISHED_HERE: Cell<Option<&'static Stack>> = const { Cell::new(None) };
}

static PUBLISHED: AtomicPtr<Stack> = AtomicPtr::new(std::ptr::null_mut());

static INTERNED: Mutex<Option<HashMap<Vec<&'static str>, &'static [&'static str]>>> = Mutex::new(None);

fn with_stack<R>(action: impl FnOnce(&Stack) -> R) -> R {
    match PUBLISHED_HERE.get() {
        Some(stack) => action(stack),
        None => LOCAL.with(action),
    }
}

fn intern(frames: Vec<&'static str>) -> &'static [&'static str] {
    let mut interned = INTERNED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let interned = interned.get_or_insert_with(HashMap::new);
    if let Some(existing) = interned.get(&frames) {
        return existing;
    }
    let leaked: &'static [&'static str] = Box::leak(frames.clone().into_boxed_slice());
    interned.insert(frames, leaked);
    leaked
}

pub struct FrameScope(std::marker::PhantomData<*const ()>);

impl Drop for FrameScope {
    fn drop(&mut self) {
        with_stack(Stack::pop);
    }
}

impl FrameScope {
    #[inline]
    pub fn switch_to(&self, frames: &'static [&'static str]) {
        with_stack(|stack| stack.replace_top(frames));
    }
}

pub fn publish_current_thread() {
    let stack: &'static Stack = Box::leak(Box::new(Stack::new()));
    for frames in LOCAL.with(|local| {
        let len = local.len.load(Ordering::Relaxed).min(CAPACITY);
        local.scopes[..len].iter().map(|scope| scope.load(Ordering::Relaxed)).collect::<Vec<_>>()
    }) {
        let (frames, absolute) = decode(frames);
        stack.push(frames, absolute);
    }
    PUBLISHED_HERE.set(Some(stack));
    PUBLISHED.store(std::ptr::from_ref(stack).cast_mut(), Ordering::Release);
}

pub fn published() -> Vec<&'static str> {
    let stack = PUBLISHED.load(Ordering::Acquire);
    if stack.is_null() {
        return Vec::new();
    }
    unsafe { &*stack }.frames()
}

pub const MANAGED_BLOCK: &str = "net.minecraft.util.thread.BlockableEventLoop.managedBlock(BlockableEventLoop.java:156)";
pub const RUN_ALL_TASKS: &str = "net.minecraft.util.thread.BlockableEventLoop.runAllTasks(BlockableEventLoop.java:127)";
pub const WAIT_UNTIL_NEXT_TICK_RUN_ALL: &str = "net.minecraft.server.MinecraftServer.waitUntilNextTick(MinecraftServer.java:856)";
pub const WAIT_UNTIL_NEXT_TICK_BLOCK: &str = "net.minecraft.server.MinecraftServer.waitUntilNextTick(MinecraftServer.java:859)";
pub const RUN_SERVER_WAIT: &str = "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:758)";
pub const RUN_SERVER_INIT: &str = "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:720)";
pub const INIT_SERVER_SAVE: &str = "net.minecraft.server.dedicated.DedicatedServer.initServer(DedicatedServer.java:251)";
pub const GET_CHUNK_BLOCK: &str = "net.minecraft.server.level.ServerChunkCache.getChunk(ServerChunkCache.java:146)";

pub const SERVER_TASK_POLL: [&str; 8] = [
    "net.minecraft.server.TickTask.run(TickTask.java:18)",
    "net.minecraft.util.thread.BlockableEventLoop.doRunTask(BlockableEventLoop.java:173)",
    "net.minecraft.util.thread.ReentrantBlockableEventLoop.doRunTask(ReentrantBlockableEventLoop.java:23)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:912)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:193)",
    "net.minecraft.util.thread.BlockableEventLoop.pollTask(BlockableEventLoop.java:147)",
    "net.minecraft.server.MinecraftServer.pollTaskInternal(MinecraftServer.java:894)",
    "net.minecraft.server.MinecraftServer.pollTask(MinecraftServer.java:888)",
];

pub const CHUNK_SOURCE_TASK: [&str; 4] = [
    "net.minecraft.util.thread.BlockableEventLoop.doRunTask(BlockableEventLoop.java:173)",
    "net.minecraft.server.level.ServerChunkCache$MainThreadExecutor.doRunTask(ServerChunkCache.java:621)",
    "net.minecraft.util.thread.BlockableEventLoop.pollTask(BlockableEventLoop.java:147)",
    "net.minecraft.server.level.ServerChunkCache$MainThreadExecutor.pollTask(ServerChunkCache.java:630)",
];

pub const SERVER_CHUNK_POLL: [&str; 3] = [
    "net.minecraft.server.level.ServerChunkCache.pollTask(ServerChunkCache.java:268)",
    "net.minecraft.server.MinecraftServer.pollTaskInternal(MinecraftServer.java:900)",
    "net.minecraft.server.MinecraftServer.pollTask(MinecraftServer.java:888)",
];

fn push(frames: &'static [&'static str], absolute: bool) -> FrameScope {
    with_stack(|stack| stack.push(frames, absolute));
    FrameScope(std::marker::PhantomData)
}

pub fn enter(frames: impl IntoIterator<Item = &'static str>) -> FrameScope {
    push(intern(frames.into_iter().collect()), false)
}

#[inline]
pub fn enter_static(frames: &'static [&'static str]) -> FrameScope {
    push(frames, false)
}

#[inline]
pub fn within<R>(frames: &'static [&'static str], run: impl FnOnce() -> R) -> R {
    let _scope = enter_static(frames);
    run()
}

pub fn enter_absolute(frames: impl IntoIterator<Item = &'static str>) -> FrameScope {
    let mut frames: Vec<&'static str> = frames.into_iter().collect();
    frames.extend(SPIN);
    push(intern(frames), true)
}

pub fn enter_absolute_static(frames: &'static [&'static str]) -> FrameScope {
    push(frames, true)
}

pub fn enter_complete(frames: Vec<&'static str>) -> FrameScope {
    push(intern(frames), true)
}

pub const PACKET_PROCESSING: [&str; 4] = [
    "net.minecraft.network.PacketProcessor$ListenerAndPacket.handle(PacketProcessor.java:55)",
    "net.minecraft.network.PacketProcessor.processQueuedPackets(PacketProcessor.java:38)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1011)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
];

pub fn current() -> Vec<&'static str> {
    with_stack(Stack::frames)
}

pub fn init_server_prepare_levels() -> FrameScope {
    enter_absolute(PREPARE_LEVELS)
}

pub fn init_server_save() -> FrameScope {
    enter_absolute([INIT_SERVER_SAVE, RUN_SERVER_INIT])
}

pub fn tick() -> FrameScope {
    enter_absolute_static(&TICK_LOOP)
}

pub fn run_server_wait() -> FrameScope {
    enter_absolute_static(&[RUN_SERVER_WAIT, SPIN[0], SPIN[1]])
}

pub fn wait_until_next_tick_run_all() -> FrameScope {
    enter_static(&[RUN_ALL_TASKS, WAIT_UNTIL_NEXT_TICK_RUN_ALL])
}

pub fn wait_until_next_tick() -> FrameScope {
    enter_static(&[MANAGED_BLOCK, WAIT_UNTIL_NEXT_TICK_BLOCK])
}

pub fn server_chunk_poll() -> FrameScope {
    enter_static(&SERVER_CHUNK_POLL)
}

pub fn chunk_source_block() -> FrameScope {
    enter_static(&[MANAGED_BLOCK, GET_CHUNK_BLOCK])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_stack_until_an_absolute_one() {
        let _base = enter_absolute(["a"]);
        {
            let _inner = enter(["b", "c"]);
            assert_eq!(&current()[..3], ["b", "c", "a"]);
            let _complete = enter_complete(vec!["x"]);
            assert_eq!(current(), ["x"]);
        }
        assert_eq!(&current()[..1], ["a"]);
        assert_eq!(current().len(), 3);
    }

    #[test]
    fn switching_a_scope_keeps_the_scopes_below() {
        let _base = enter_absolute_static(&["a"]);
        let phase = enter_static(&["b"]);
        phase.switch_to(&["c", "d"]);
        assert_eq!(current(), ["c", "d", "a"]);
        drop(phase);
        assert_eq!(current(), ["a"]);
    }
}
