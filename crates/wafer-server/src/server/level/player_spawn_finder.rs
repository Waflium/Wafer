use wafer_core::{AABB, BlockPos, ChunkPos, Direction, Vec3, mth};
use rand::RngExt;

use super::ServerLevel;
use super::game_mode::GameType;
use super::ticket::TicketType;
use crate::crash_report::CrashReport;
use crate::server::Server;
use crate::server::main_thread_frames;
use crate::util::java_stack;
use crate::util::java_throwable::{JavaThrowable, ThrowableReport};
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;

const ABSOLUTE_MAX_ATTEMPTS: i64 = 1024;
const PLAYER_WIDTH: f64 = 0.6;
const PLAYER_HEIGHT: f64 = 1.8;

const SCHEDULE_CANDIDATE_FOR_THROWABLE: &str = "net.minecraft.server.level.PlayerSpawnFinder.lambda$scheduleCandidate$0(PlayerSpawnFinder.java:143)";
const SCHEDULE_CANDIDATE_ADD_CATEGORY: &str = "net.minecraft.server.level.PlayerSpawnFinder.lambda$scheduleCandidate$0(PlayerSpawnFinder.java:144)";
const SCHEDULE_CANDIDATE_REPORTED: &str = "net.minecraft.server.level.PlayerSpawnFinder.lambda$scheduleCandidate$0(PlayerSpawnFinder.java:149)";
const UNI_WHEN_COMPLETE_RUN: [&str; 3] = [
    "java.base/java.util.concurrent.CompletableFuture.uniWhenComplete(CompletableFuture.java:884)",
    "java.base/java.util.concurrent.CompletableFuture$UniWhenComplete.tryFire(CompletableFuture.java:862)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.run(CompletableFuture.java:503)",
];
const EXECUTE_ON_SERVER_THREAD: [&str; 6] = [
    "net.minecraft.server.TickTask.run(TickTask.java:18)",
    "net.minecraft.util.thread.BlockableEventLoop.doRunTask(BlockableEventLoop.java:173)",
    "net.minecraft.util.thread.ReentrantBlockableEventLoop.doRunTask(ReentrantBlockableEventLoop.java:23)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:912)",
    "net.minecraft.server.MinecraftServer.doRunTask(MinecraftServer.java:193)",
    "net.minecraft.util.thread.BlockableEventLoop.execute(BlockableEventLoop.java:114)",
];
const FULL_CHUNK_COMPLETED: [&str; 11] = [
    "java.base/java.util.concurrent.CompletableFuture$UniCompletion.claim(CompletableFuture.java:593)",
    "java.base/java.util.concurrent.CompletableFuture.uniWhenComplete(CompletableFuture.java:875)",
    "java.base/java.util.concurrent.CompletableFuture$UniWhenComplete.tryFire(CompletableFuture.java:862)",
    "java.base/java.util.concurrent.CompletableFuture.postComplete(CompletableFuture.java:531)",
    "java.base/java.util.concurrent.CompletableFuture.complete(CompletableFuture.java:2206)",
    "net.minecraft.server.level.GenerationChunkHolder.completeFuture(GenerationChunkHolder.java:213)",
    "net.minecraft.server.level.GenerationChunkHolder.lambda$applyStep$0(GenerationChunkHolder.java:92)",
    "java.base/java.util.concurrent.CompletableFuture.uniHandle(CompletableFuture.java:955)",
    "java.base/java.util.concurrent.CompletableFuture$UniHandle.tryFire(CompletableFuture.java:932)",
    "java.base/java.util.concurrent.CompletableFuture.postComplete(CompletableFuture.java:531)",
    "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1794)",
];
const WHEN_COMPLETE_ASYNC_DONE: [&str; 3] = [
    "java.base/java.util.concurrent.CompletableFuture.uniWhenCompleteStage(CompletableFuture.java:911)",
    "java.base/java.util.concurrent.CompletableFuture.whenCompleteAsync(CompletableFuture.java:2394)",
    "net.minecraft.server.level.PlayerSpawnFinder.scheduleCandidate(PlayerSpawnFinder.java:129)",
];
const SCHEDULE_NEXT_CANDIDATE: &str = "net.minecraft.server.level.PlayerSpawnFinder.scheduleNext(PlayerSpawnFinder.java:83)";
const SCHEDULE_NEXT_FIXUP: &str = "net.minecraft.server.level.PlayerSpawnFinder.scheduleNext(PlayerSpawnFinder.java:91)";
const FIND_SPAWN_SCHEDULE_NEXT: &str = "net.minecraft.server.level.PlayerSpawnFinder.findSpawn(PlayerSpawnFinder.java:70)";
const SCHEDULE_CANDIDATE_SCHEDULE_NEXT: &str = "net.minecraft.server.level.PlayerSpawnFinder.lambda$scheduleCandidate$0(PlayerSpawnFinder.java:136)";
const JOIN: [&str; 3] = [
    "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
    "java.base/java.util.concurrent.CompletableFuture.reportJoin(CompletableFuture.java:457)",
    "java.base/java.util.concurrent.CompletableFuture.join(CompletableFuture.java:2139)",
];
pub const ADJUST_SPAWN_LOCATION_FIND_SPAWN: &str = "net.minecraft.server.level.ServerPlayer.adjustSpawnLocation(ServerPlayer.java:423)";
const ADJUST_SPAWN_LOCATION_JOIN: &str = "net.minecraft.server.level.ServerPlayer.adjustSpawnLocation(ServerPlayer.java:425)";

#[derive(Clone, Copy)]
enum Check {
    Candidate,
    FixupSpawnHeight,
}

#[derive(Clone)]
enum Callback {
    ChunkLoaded,
    Queued,
    Inline(Vec<&'static str>),
}

#[derive(Clone)]
struct Candidate {
    x: i32,
    z: i32,
    index: i32,
    check: Check,
    callback: Callback,
}

#[derive(Clone)]
pub enum Finished {
    Spawn(Vec3),
    Failed(Box<JavaThrowable>),
    Cancelled,
}

pub struct PlayerSpawnFinder {
    level: usize,
    spawn_suggestion: BlockPos,
    radius: i32,
    candidate_count: i32,
    coprime: i32,
    offset: i32,
    next_candidate_index: i32,
    pending: Option<Candidate>,
    finished: Option<Finished>,
}

impl PlayerSpawnFinder {
    fn new(level: usize, spawn_suggestion: BlockPos, radius: i32) -> PlayerSpawnFinder {
        let square_side = radius as i64 * 2 + 1;
        let candidate_count = ABSOLUTE_MAX_ATTEMPTS.min(square_side * square_side) as i32;
        PlayerSpawnFinder {
            level,
            spawn_suggestion,
            radius,
            candidate_count,
            coprime: get_coprime(candidate_count),
            offset: rand::rng().random_range(0..candidate_count),
            next_candidate_index: 0,
            pending: None,
            finished: None,
        }
    }

    fn completed(level: usize, spawn_suggestion: BlockPos, position: Vec3) -> PlayerSpawnFinder {
        PlayerSpawnFinder {
            level,
            spawn_suggestion,
            radius: 0,
            candidate_count: 0,
            coprime: 0,
            offset: 0,
            next_candidate_index: 0,
            pending: None,
            finished: Some(Finished::Spawn(position)),
        }
    }

    pub fn finished(&self) -> Option<&Finished> {
        self.finished.as_ref()
    }

    pub fn into_finished(self) -> Option<Finished> {
        self.finished
    }

    pub fn cancel(&mut self) {
        self.finished.get_or_insert(Finished::Cancelled);
    }
}

fn callback_frames(callback: &Callback) -> Vec<&'static str> {
    let mut frames = UNI_WHEN_COMPLETE_RUN.to_vec();
    match callback {
        Callback::ChunkLoaded => {
            frames.extend(EXECUTE_ON_SERVER_THREAD);
            frames.extend(FULL_CHUNK_COMPLETED);
            frames.extend(main_thread_frames::CHUNK_SOURCE_TASK);
            frames.extend(main_thread_frames::SERVER_CHUNK_POLL);
            frames.extend(main_thread_frames::current());
        }
        Callback::Queued => {
            frames.extend(main_thread_frames::SERVER_TASK_POLL);
            frames.extend(main_thread_frames::current());
        }
        Callback::Inline(caller) => {
            frames.extend(EXECUTE_ON_SERVER_THREAD);
            frames.extend_from_slice(caller);
        }
    }
    frames
}

fn searching_for_spawn_exception(cause: JavaThrowable, callback: Vec<&'static str>, details: Vec<(String, String)>) -> JavaThrowable {
    let at = |frame: &'static str| std::iter::once(frame).chain(callback.iter().copied()).collect::<Vec<_>>();
    let _creator = java_stack::enter_all(&at(SCHEDULE_CANDIDATE_FOR_THROWABLE));
    let report = ThrowableReport {
        title: "Searching for spawn".to_string(),
        category: "Spawn Lookup".to_string(),
        details,
        category_stack_trace: at(SCHEDULE_CANDIDATE_ADD_CATEGORY),
        system_report: None,
    };
    let mut reported = JavaThrowable::new("net.minecraft.ReportedException: Searching for spawn", at(SCHEDULE_CANDIDATE_REPORTED)).with_cause(cause).with_report(report);
    CrashReport::create_for_reported(&mut reported);
    reported
}

pub(crate) fn completion_exception(reported: &JavaThrowable, frames: Vec<&'static str>) -> JavaThrowable {
    let mut stack_trace = JOIN.to_vec();
    stack_trace.extend(frames);
    JavaThrowable::new(format!("java.util.concurrent.CompletionException: {}", reported.description), stack_trace).with_cause(reported.clone())
}

fn get_coprime(possible_origins: i32) -> i32 {
    if possible_origins <= 16 { possible_origins - 1 } else { 17 }
}

impl Server {
    pub(crate) fn find_spawn(&mut self, level: usize, spawn_suggestion: BlockPos) -> u64 {
        self.next_spawn_finder += 1;
        let id = self.next_spawn_finder;
        if self.default_game_type.get() == GameType::Adventure {
            let position = self.levels[level].fixup_spawn_height(spawn_suggestion);
            self.spawn_finders.insert(id, PlayerSpawnFinder::completed(level, spawn_suggestion, position));
            return id;
        }
        let mut radius = self.levels[level].game_rules.get_int(wafer_data::game_rule::RESPAWN_RADIUS).max(0);
        let dist_to_border = mth::floor(self.levels[level].world_border.get_distance_to_border(spawn_suggestion.x as f64, spawn_suggestion.z as f64));
        if dist_to_border < radius {
            radius = dist_to_border;
        }
        if dist_to_border <= 1 {
            radius = 1;
        }
        self.spawn_finders.insert(id, PlayerSpawnFinder::new(level, spawn_suggestion, radius));
        self.schedule_next_spawn_candidate(id, || {
            let mut caller = vec![FIND_SPAWN_SCHEDULE_NEXT];
            caller.extend(main_thread_frames::current());
            caller
        });
        id
    }

    pub(crate) fn find_spawn_blocking(&mut self, level: usize, spawn_suggestion: BlockPos) -> Vec3 {
        let id = main_thread_frames::within(&[ADJUST_SPAWN_LOCATION_FIND_SPAWN], || self.find_spawn(level, spawn_suggestion));
        let _join = main_thread_frames::enter_static(&[ADJUST_SPAWN_LOCATION_JOIN]);
        self.join_spawn_finder(id)
    }

    pub(crate) fn join_spawn_finder(&mut self, id: u64) -> Vec3 {
        self.managed_block(|server| server.spawn_finders.get(&id).is_some_and(|finder| finder.finished().is_some()));
        match self.spawn_finders.remove(&id).and_then(PlayerSpawnFinder::into_finished) {
            Some(Finished::Spawn(position)) => position,
            Some(Finished::Failed(reported)) => crate::crash_report::throw_java_exception(completion_exception(&reported, main_thread_frames::current())),
            Some(Finished::Cancelled) | None => unreachable!("a joined spawn search is never cancelled"),
        }
    }

    fn schedule_next_spawn_candidate(&mut self, id: u64, caller: impl FnOnce() -> Vec<&'static str>) {
        let Some(finder) = self.spawn_finders.get_mut(&id) else { return };
        let candidate_index = finder.next_candidate_index;
        finder.next_candidate_index += 1;
        let (x, z, check, schedule_next) = if candidate_index < finder.candidate_count {
            let value = (finder.offset + finder.coprime * candidate_index) % finder.candidate_count;
            let delta_x = value % (finder.radius * 2 + 1);
            let delta_z = value / (finder.radius * 2 + 1);
            (finder.spawn_suggestion.x + delta_x - finder.radius, finder.spawn_suggestion.z + delta_z - finder.radius, Check::Candidate, SCHEDULE_NEXT_CANDIDATE)
        } else {
            (finder.spawn_suggestion.x, finder.spawn_suggestion.z, Check::FixupSpawnHeight, SCHEDULE_NEXT_FIXUP)
        };
        self.schedule_spawn_candidate(id, x, z, candidate_index, check, || {
            let mut frames = WHEN_COMPLETE_ASYNC_DONE.to_vec();
            frames.push(schedule_next);
            frames.extend(caller());
            frames
        });
    }

    fn schedule_spawn_candidate(&mut self, id: u64, x: i32, z: i32, index: i32, check: Check, caller: impl FnOnce() -> Vec<&'static str>) {
        let Some(finder) = self.spawn_finders.get(&id) else { return };
        if finder.finished.is_some() {
            return;
        }
        let level = finder.level;
        let chunks = &mut self.levels[level].chunks;
        let future = chunks.add_ticket_and_load_with_radius(TicketType::SpawnSearch, ChunkPos::new(x >> 4, z >> 4), 0);
        if !chunks.when_complete_run_spawn_search(future, id) {
            self.set_pending_spawn_candidate(id, Candidate { x, z, index, check, callback: Callback::ChunkLoaded });
            return;
        }
        let callback = if self.running_task() { Callback::Queued } else { Callback::Inline(caller()) };
        self.set_pending_spawn_candidate(id, Candidate { x, z, index, check, callback });
    }

    fn set_pending_spawn_candidate(&mut self, id: u64, candidate: Candidate) {
        if let Some(finder) = self.spawn_finders.get_mut(&id) {
            finder.pending = Some(candidate);
        }
    }

    pub(crate) fn run_spawn_search_step(&mut self, id: u64) {
        let Some(finder) = self.spawn_finders.get_mut(&id) else { return };
        let Some(candidate) = finder.pending.take() else { return };
        let (level, spawn_suggestion) = (finder.level, finder.spawn_suggestion);
        let checked = crate::util::catch_throwable(|| {
            let level = &mut self.levels[level];
            match candidate.check {
                Check::Candidate => get_level_respawn_pos(level, candidate.x, candidate.z).filter(|&spawn_pos| level.no_collision_no_liquid(spawn_pos)).map(Vec3::at_bottom_center_of),
                Check::FixupSpawnHeight => Some(level.fixup_spawn_height(spawn_suggestion)),
            }
        });
        match checked {
            Ok(Some(position)) => {
                if let Some(finder) = self.spawn_finders.get_mut(&id) {
                    finder.finished.get_or_insert(Finished::Spawn(position));
                }
            }
            Ok(None) => self.schedule_next_spawn_candidate(id, || {
                let mut caller = vec![SCHEDULE_CANDIDATE_SCHEDULE_NEXT];
                caller.extend(callback_frames(&candidate.callback));
                caller
            }),
            Err(cause) => {
                let finder = &self.spawn_finders[&id];
                let details = vec![
                    ("Origin".to_string(), format!("BlockPos{{x={}, y={}, z={}}}", spawn_suggestion.x, spawn_suggestion.y, spawn_suggestion.z)),
                    ("Radius".to_string(), finder.radius.to_string()),
                    ("Candidate".to_string(), format!("[{},{}]", candidate.x, candidate.z)),
                    ("Progress".to_string(), format!("{} out of {}", candidate.index, finder.candidate_count)),
                ];
                let reported = searching_for_spawn_exception(cause, callback_frames(&candidate.callback), details);
                if let Some(finder) = self.spawn_finders.get_mut(&id) {
                    finder.finished.get_or_insert(Finished::Failed(Box::new(reported)));
                }
            }
        }
    }
}

impl ServerLevel {
    fn fixup_spawn_height(&mut self, spawn_pos: BlockPos) -> Vec3 {
        let mut pos = spawn_pos;
        while !self.no_collision_no_liquid(pos) && pos.y < self.chunks.max_y() {
            pos = pos.above();
        }
        pos = pos.below();
        while self.no_collision_no_liquid(pos) && pos.y > self.chunks.min_y() {
            pos = pos.below();
        }
        Vec3::at_bottom_center_of(pos.above())
    }

    fn no_collision_no_liquid(&self, pos: BlockPos) -> bool {
        let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
        let half_width = PLAYER_WIDTH / 2.0;
        let aabb = AABB::new(x - half_width, y, z - half_width, x + half_width, y + PLAYER_HEIGHT, z + half_width);
        self.no_block_and_liquid_collision(&aabb) && self.entity_collisions(None, &aabb).is_empty()
    }
}

pub fn initial_spawn(level: &mut ServerLevel) -> BlockPos {
    let spawn_chunk = level.chunks.generator.origin();
    let mut height = level.chunks.generator.spawn_height(level.chunks.min_y(), level.chunks.height());
    if height < level.chunks.min_y() {
        let (x, z) = (spawn_chunk.min_block_x() + 8, spawn_chunk.min_block_z() + 8);
        height = level.get_chunk(spawn_chunk).height_at(HeightmapType::WorldSurface, x, z);
    }
    let mut spawn = BlockPos::new(spawn_chunk.min_block_x() + 8, height, spawn_chunk.min_block_z() + 8);
    let (mut x_chunk_offset, mut z_chunk_offset, mut dx_chunk, mut dz_chunk) = (0, 0, 0, -1);
    for _ in 0..11 * 11 {
        if (-5..=5).contains(&x_chunk_offset)
            && (-5..=5).contains(&z_chunk_offset)
            && let Some(tested_pos) = spawn_pos_in_chunk(level, ChunkPos::new(spawn_chunk.x + x_chunk_offset, spawn_chunk.z + z_chunk_offset))
        {
            spawn = tested_pos;
            break;
        }
        if x_chunk_offset == z_chunk_offset || (x_chunk_offset < 0 && x_chunk_offset == -z_chunk_offset) || (x_chunk_offset > 0 && x_chunk_offset == 1 - z_chunk_offset) {
            (dx_chunk, dz_chunk) = (-dz_chunk, dx_chunk);
        }
        x_chunk_offset += dx_chunk;
        z_chunk_offset += dz_chunk;
    }
    spawn
}

fn spawn_pos_in_chunk(level: &mut ServerLevel, pos: ChunkPos) -> Option<BlockPos> {
    for x in pos.min_block_x()..pos.min_block_x() + 16 {
        for z in pos.min_block_z()..pos.min_block_z() + 16 {
            if let Some(spawn) = get_level_respawn_pos(level, x, z) {
                return Some(spawn);
            }
        }
    }
    None
}

fn get_level_respawn_pos(level: &mut ServerLevel, x: i32, z: i32) -> Option<BlockPos> {
    let min_y = level.chunks.min_y();
    let cave_world = level.chunks.dimension.has_ceiling;
    let spawn_height = level.chunks.generator.spawn_height(min_y, level.chunks.height());
    let chunk = level.get_chunk(ChunkPos::new(x >> 4, z >> 4));
    let top = if cave_world { spawn_height } else { chunk.height_at(HeightmapType::MotionBlocking, x, z) };
    if top < min_y {
        return None;
    }
    let surface = chunk.height_at(HeightmapType::WorldSurface, x, z);
    if surface <= top && surface > chunk.height_at(HeightmapType::OceanFloor, x, z) {
        return None;
    }
    for y in (min_y..=top + 1).rev() {
        let pos = BlockPos::new(x, y, z);
        let state = level.block_state(pos);
        if !state.fluid_state().is_empty() {
            break;
        }
        if wafer_core::shapes::is_face_full(&level.get_state_collision_shape(state, pos), Direction::Up) {
            return Some(BlockPos::new(x, y + 1, z));
        }
    }
    None
}
