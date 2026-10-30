use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::sync::Mutex;

use super::command_frames::{PREPARE_LEVELS, SPIN, TICK_LOOP};

struct Scope {
    frames: Cow<'static, [&'static str]>,
    absolute: bool,
}

thread_local! {
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
    static PUBLISHING: Cell<bool> = const { Cell::new(false) };
}

static PUBLISHED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

pub struct FrameScope(());

impl Drop for FrameScope {
    fn drop(&mut self) {
        SCOPES.with(|scopes| scopes.borrow_mut().pop());
        publish();
    }
}

pub fn publish_current_thread() {
    PUBLISHING.set(true);
    publish();
}

fn publish() {
    if PUBLISHING.get() {
        *PUBLISHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = current();
    }
}

pub fn published() -> Vec<&'static str> {
    PUBLISHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
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

fn push(frames: Cow<'static, [&'static str]>, absolute: bool) -> FrameScope {
    SCOPES.with(|scopes| scopes.borrow_mut().push(Scope { frames, absolute }));
    publish();
    FrameScope(())
}

pub fn enter(frames: impl IntoIterator<Item = &'static str>) -> FrameScope {
    push(Cow::Owned(frames.into_iter().collect()), false)
}

pub fn enter_static(frames: &'static [&'static str]) -> FrameScope {
    push(Cow::Borrowed(frames), false)
}

pub fn enter_absolute(frames: impl IntoIterator<Item = &'static str>) -> FrameScope {
    let mut frames: Vec<&'static str> = frames.into_iter().collect();
    frames.extend(SPIN);
    push(Cow::Owned(frames), true)
}

pub fn enter_complete(frames: Vec<&'static str>) -> FrameScope {
    push(Cow::Owned(frames), true)
}

pub const PACKET_PROCESSING: [&str; 4] = [
    "net.minecraft.network.PacketProcessor$ListenerAndPacket.handle(PacketProcessor.java:55)",
    "net.minecraft.network.PacketProcessor.processQueuedPackets(PacketProcessor.java:38)",
    "net.minecraft.server.MinecraftServer.processPacketsAndTick(MinecraftServer.java:1011)",
    "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:752)",
];

pub fn current() -> Vec<&'static str> {
    SCOPES.with(|scopes| {
        let mut frames = Vec::new();
        for scope in scopes.borrow().iter().rev() {
            frames.extend_from_slice(&scope.frames);
            if scope.absolute {
                return frames;
            }
        }
        frames.extend(SPIN);
        frames
    })
}

pub fn init_server_prepare_levels() -> FrameScope {
    enter_absolute(PREPARE_LEVELS)
}

pub fn init_server_save() -> FrameScope {
    enter_absolute([INIT_SERVER_SAVE, RUN_SERVER_INIT])
}

pub fn tick() -> FrameScope {
    enter_absolute(TICK_LOOP[..4].iter().copied())
}

pub fn run_all_tasks() -> FrameScope {
    enter_absolute([RUN_ALL_TASKS, WAIT_UNTIL_NEXT_TICK_RUN_ALL, RUN_SERVER_WAIT])
}

pub fn running_wait() -> FrameScope {
    enter_absolute([MANAGED_BLOCK, WAIT_UNTIL_NEXT_TICK_BLOCK, RUN_SERVER_WAIT])
}

pub fn wait_until_next_tick() -> FrameScope {
    enter([MANAGED_BLOCK, WAIT_UNTIL_NEXT_TICK_BLOCK])
}

pub fn server_chunk_poll() -> FrameScope {
    enter(SERVER_CHUNK_POLL)
}

pub fn chunk_source_block() -> FrameScope {
    enter([MANAGED_BLOCK, GET_CHUNK_BLOCK])
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
}
