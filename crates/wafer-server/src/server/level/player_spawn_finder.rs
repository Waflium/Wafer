use wafer_core::{AABB, BlockPos, ChunkPos, Direction, Vec3, mth};
use rand::RngExt;

use super::ServerLevel;
use super::game_mode::GameType;
use super::ticket::TicketType;
use crate::server::Server;
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;

const ABSOLUTE_MAX_ATTEMPTS: i64 = 1024;
const PLAYER_WIDTH: f64 = 0.6;
const PLAYER_HEIGHT: f64 = 1.8;

#[derive(Clone, Copy)]
enum Check {
    Candidate,
    FixupSpawnHeight,
}

#[derive(Clone, Copy)]
struct Candidate {
    x: i32,
    z: i32,
    index: i32,
    check: Check,
}

#[derive(Clone, Copy)]
pub enum Finished {
    Spawn(Vec3),
    Failed,
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

    pub fn finished(&self) -> Option<Finished> {
        self.finished
    }

    pub fn cancel(&mut self) {
        self.finished.get_or_insert(Finished::Failed);
    }
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
        self.schedule_next_spawn_candidate(id);
        id
    }

    pub(crate) fn find_spawn_blocking(&mut self, level: usize, spawn_suggestion: BlockPos) -> Vec3 {
        let id = self.find_spawn(level, spawn_suggestion);
        self.join_spawn_finder(id)
    }

    pub(crate) fn join_spawn_finder(&mut self, id: u64) -> Vec3 {
        self.managed_block(|server| server.spawn_finders.get(&id).is_some_and(|finder| finder.finished().is_some()));
        match self.spawn_finders.remove(&id).and_then(|finder| finder.finished()) {
            Some(Finished::Spawn(position)) => position,
            _ => panic!("java.util.concurrent.CompletionException: Searching for spawn"),
        }
    }

    fn schedule_next_spawn_candidate(&mut self, id: u64) {
        let Some(finder) = self.spawn_finders.get_mut(&id) else { return };
        let candidate_index = finder.next_candidate_index;
        finder.next_candidate_index += 1;
        let candidate = if candidate_index < finder.candidate_count {
            let value = (finder.offset + finder.coprime * candidate_index) % finder.candidate_count;
            let delta_x = value % (finder.radius * 2 + 1);
            let delta_z = value / (finder.radius * 2 + 1);
            Candidate { x: finder.spawn_suggestion.x + delta_x - finder.radius, z: finder.spawn_suggestion.z + delta_z - finder.radius, index: candidate_index, check: Check::Candidate }
        } else {
            Candidate { x: finder.spawn_suggestion.x, z: finder.spawn_suggestion.z, index: candidate_index, check: Check::FixupSpawnHeight }
        };
        self.schedule_spawn_candidate(id, candidate);
    }

    fn schedule_spawn_candidate(&mut self, id: u64, candidate: Candidate) {
        let Some(finder) = self.spawn_finders.get_mut(&id) else { return };
        if finder.finished.is_some() {
            return;
        }
        finder.pending = Some(candidate);
        let chunks = &mut self.levels[finder.level].chunks;
        let future = chunks.add_ticket_and_load_with_radius(TicketType::SpawnSearch, ChunkPos::new(candidate.x >> 4, candidate.z >> 4), 0);
        chunks.when_complete_run_spawn_search(future, id);
    }

    pub(crate) fn run_spawn_search_step(&mut self, id: u64) {
        let Some(finder) = self.spawn_finders.get_mut(&id) else { return };
        let Some(candidate) = finder.pending.take() else { return };
        let (level, spawn_suggestion) = (finder.level, finder.spawn_suggestion);
        let checked = crate::util::catch_exception(|| {
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
            Ok(None) => self.schedule_next_spawn_candidate(id),
            Err(exception) => {
                error!(
                    "Searching for spawn\nSpawn Lookup\nOrigin: BlockPos{{x={}, y={}, z={}}}\nRadius: {}\nCandidate: [{},{}]\nProgress: {} out of {}\n{exception}",
                    spawn_suggestion.x, spawn_suggestion.y, spawn_suggestion.z, self.spawn_finders[&id].radius, candidate.x, candidate.z, candidate.index, self.spawn_finders[&id].candidate_count
                );
                if let Some(finder) = self.spawn_finders.get_mut(&id) {
                    finder.finished.get_or_insert(Finished::Failed);
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
        self.no_block_and_liquid_collision(&AABB::new(x - half_width, y, z - half_width, x + half_width, y + PLAYER_HEIGHT, z + half_width))
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
