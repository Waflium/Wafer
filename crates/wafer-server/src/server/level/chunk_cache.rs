use wafer_core::hash_map::{FastMap, FastSet};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use indexmap::IndexSet;

use wafer_core::{BlockPos, ChunkPos, Vec3};
use wafer_data::{Block, BlockState, DimensionType, Fluid, blocks};
use wafer_protocol::{Writer, stream_codec};

use super::chunk_holder::{ChunkGenerationTask, ChunkHolders, FutureId, RangeThen, RangeWait, UNLOADED, Waiter};
use super::chunk_level::{self, ChunkPyramid, ChunkStatus, FullChunkStatus};
use super::chunk_task_dispatcher::{ChunkTaskPriorityQueue, MainThreadTask};
use super::chunk_workers::{self, ChunkToSave, IoReporting, IoWorker, MainThreadWakeup, ResultSender, WorkResult};
use super::distance_manager::DistanceManager;
use super::threaded_level_light_engine::{LightTask, TaskType, ThreadedLevelLightEngine};
use super::ticket::{Ticket, TicketType};
use super::SoundSource;
use crate::server::{ServerTasks, Task};
use crate::util::profiling::profiler;
use crate::world::difficulty::Difficulty;
use crate::world::scores::scoreboard::Scoreboard;
use crate::world::entity::ai::village::poi::poi_manager::PoiManager;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::debug_level_source;
use crate::world::level::chunk::serialization::{self, SectionLight};
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::chunk::storage::chunk_io_error_reporter::{ChunkIoErrorReporter, chunk_load_failure_cause};
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;
use crate::util::java_throwable::JavaThrowable;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::chunk::block_entities::{BlockEntityTickers, EntityCreationType};
use super::world_gen_region::{RegionLevelData, RegionLightTask, WorldGenRegion};
use crate::world::attribute::EnvironmentAttributeSystem;
use crate::util::datafix::data_fix_types::{DataFixTypes, current_version};
use crate::world::level::chunk::storage::simple_region_storage;
use crate::world::level::gamerules::GameRules;
use crate::world::level::natural_spawner::SpawnState;
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::chunk::{Biome, ChunkGenerator, ChunkStructures, HeightmapType, LevelChunk};
use crate::world::level::levelgen::beardifier::Beardifier;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::terrain_adjustment::TerrainAdjustment;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use std::sync::Arc;
use crate::world::level::levelgen::material::WorldGenerationContext;
use crate::world::level::levelgen::random_state::RandomState;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::lighting::{self, section};
use crate::world::ticks::{LevelChunkTicks, LevelTicks};

struct Job {
    pos: ChunkPos,
    holder: u64,
    status: ChunkStatus,
    work: FutureId,
}

pub struct ChunkCache {
    pub dimension: &'static DimensionType,
    pub generator: ChunkGenerator,
    pub data_fix_context: Option<wafer_protocol::nbt::Nbt>,
    chunks: FastMap<ChunkPos, LevelChunk>,
    terrain_chunks: FastMap<ChunkPos, LevelChunk>,
    structure_chunks: FastMap<ChunkPos, ChunkStructures>,
    loaded_structure_chunks: FastSet<ChunkPos>,
    biome_chunks: FastMap<ChunkPos, Arc<crate::world::level::levelgen::chunk_generator::ChunkBiomes>>,
    region_cache_for_blender: crate::util::FastIndexMap<i64, Arc<Vec<bool>>>,
    structure_state: Option<Arc<ChunkGeneratorStructureState>>,
    region_random_state: Option<Arc<RandomState>>,
    pub is_debug: bool,
    pub generate_structures: bool,
    pub seed: i64,
    pub difficulty: Difficulty,
    pub game_rules: GameRules,
    pub region_level_data: RegionLevelData,
    pub unloaded_block_entities: Vec<(BlockPos, crate::world::level::block::entity::BlockEntity)>,
    static_environment_attributes: EnvironmentAttributeSystem,
    pub spawn_enemies: bool,
    pub last_spawn_state: Option<SpawnState>,
    io: Option<IoWorker>,
    pub light_engine: ThreadedLevelLightEngine,
    ticking_light_changes: Vec<(LightLayer, i64)>,
    pub block_ticks: LevelTicks<Block>,
    pub fluid_ticks: LevelTicks<Fluid>,
    pub game_time: i64,
    pub distance_manager: DistanceManager,
    holders: ChunkHolders,
    worldgen_queue: ChunkTaskPriorityQueue<u64>,
    results: (ResultSender, Receiver<WorkResult>),
    jobs: HashMap<u64, Job>,
    next_token: u64,
    in_flight: VecDeque<u64>,
    arrived: HashMap<u64, WorkResult>,
    unload_queue: VecDeque<i64>,
    waiting_unloads: IndexSet<i64>,
    next_chunk_save_time: FastMap<i64, Instant>,
    chunks_to_eagerly_save: IndexSet<i64>,
    unsaved_listener: (std::sync::mpsc::Sender<i64>, Receiver<i64>),
    loaded_results: HashMap<u64, io::Result<Option<serialization::ReadChunk>>>,
    ticking_ready: Vec<(ChunkPos, FutureId)>,
    chunks_ready_to_send: Vec<ChunkPos>,
    structure_starts_available: Vec<(ChunkPos, FastMap<StructureHolder, i32>)>,
    pub(crate) server_tasks: Option<(ServerTasks, usize)>,
    world_gen_chunk_entities: Vec<(ChunkPos, Vec<wafer_protocol::nbt::Nbt>)>,
    legacy_chunk_entities: Vec<(ChunkPos, Vec<wafer_protocol::nbt::Nbt>)>,
    world_gen_entity_teams: Vec<(u128, String)>,
    world_gen_entity_sounds: Vec<(Vec3, i32, SoundSource, f32, f32)>,
    pub scoreboard: Rc<RefCell<Scoreboard>>,
    full_status_changes: VecDeque<(ChunkPos, FullChunkStatus)>,
    last_chunk: Cell<[Option<(ChunkPos, ChunkStatus, bool)>; 4]>,
    server_view_distance: i32,
    live_futures: usize,
    pub block_entity_tickers: BlockEntityTickers,
    pub tick_deadline: Option<Instant>,
    pub poi_manager: PoiManager,
    reporting: Option<IoReporting>,
}

impl ChunkCache {
    pub fn new(dimension: &'static DimensionType, generator: impl Into<ChunkGenerator>, storage: Option<RegionStorage>) -> ChunkCache {
        let generator = generator.into();
        let seed = match &generator {
            ChunkGenerator::Noise(noise) => noise.seed(),
            ChunkGenerator::Flat(_) | ChunkGenerator::Debug(_) => 0,
        };
        let structure_state = None;
        let mut cache = ChunkCache {
            dimension,
            generator,
            data_fix_context: None,
            chunks: FastMap::default(),
            terrain_chunks: FastMap::default(),
            structure_chunks: FastMap::default(),
            loaded_structure_chunks: FastSet::default(),
            biome_chunks: FastMap::default(),
            region_cache_for_blender: Default::default(),
            structure_state,
            region_random_state: None,
            is_debug: false,
            generate_structures: true,
            seed: 0,
            difficulty: Difficulty::Easy,
            game_rules: GameRules::default(),
            region_level_data: RegionLevelData::default(),
            unloaded_block_entities: Vec::new(),
            static_environment_attributes: EnvironmentAttributeSystem::builder().add_static_layers(dimension).build(),
            spawn_enemies: true,
            last_spawn_state: None,
            io: storage.map(IoWorker::new),
            light_engine: ThreadedLevelLightEngine::new(dimension.min_y, dimension.height as i32, dimension.has_skylight),
            ticking_light_changes: Vec::new(),
            block_ticks: LevelTicks::default(),
            fluid_ticks: LevelTicks::default(),
            game_time: 0,
            distance_manager: DistanceManager::default(),
            holders: ChunkHolders::default(),
            worldgen_queue: ChunkTaskPriorityQueue::default(),
            results: chunk_workers::results(),
            jobs: HashMap::new(),
            next_token: 0,
            in_flight: VecDeque::new(),
            arrived: HashMap::new(),
            unload_queue: VecDeque::new(),
            waiting_unloads: IndexSet::new(),
            next_chunk_save_time: FastMap::default(),
            chunks_to_eagerly_save: IndexSet::new(),
            unsaved_listener: std::sync::mpsc::channel(),
            loaded_results: HashMap::new(),
            ticking_ready: Vec::new(),
            chunks_ready_to_send: Vec::new(),
            structure_starts_available: Vec::new(),
            server_tasks: None,
            world_gen_chunk_entities: Vec::new(),
            legacy_chunk_entities: Vec::new(),
            world_gen_entity_teams: Vec::new(),
            world_gen_entity_sounds: Vec::new(),
            scoreboard: Rc::default(),
            full_status_changes: VecDeque::new(),
            last_chunk: Cell::new([None; 4]),
            server_view_distance: 0,
            live_futures: 0,
            tick_deadline: None,
            block_entity_tickers: BlockEntityTickers::default(),
            poi_manager: PoiManager::new(None, dimension.min_y >> 4, (dimension.min_y + dimension.height as i32 - 1) >> 4),
            reporting: None,
        };
        cache.set_seed(seed);
        cache.set_view_distance(10);
        cache
    }

    pub fn set_poi_storage(&mut self, storage: RegionStorage) {
        let (min_section_y, max_section_y) = self.poi_manager.section_range();
        self.poi_manager = PoiManager::new(Some(IoWorker::new(storage)), min_section_y, max_section_y);
    }

    pub fn set_error_reporting(&mut self, reporter: ChunkIoErrorReporter, level_id: &str, dimension: &str) {
        let reporting = IoReporting { reporter: reporter.clone(), info: RegionStorageInfo::new(level_id, dimension, "chunk") };
        if let Some(io) = &self.io {
            io.set_reporting(reporting.clone());
        }
        self.reporting = Some(reporting);
        self.poi_manager.storage.set_reporting(IoReporting { reporter, info: RegionStorageInfo::new(level_id, dimension, "poi") });
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = seed;
        let structure_state = match &self.generator {
            ChunkGenerator::Noise(_) | ChunkGenerator::Debug(_) => ChunkGeneratorStructureState::create_for_normal(&self.generator, seed),
            ChunkGenerator::Flat(flat) => ChunkGeneratorStructureState::create_for_flat(flat, seed),
        };
        self.structure_state = Some(Arc::new(structure_state));
        self.region_random_state = match &self.generator {
            ChunkGenerator::Noise(_) => None,
            ChunkGenerator::Flat(_) | ChunkGenerator::Debug(_) => Some(Arc::new(RandomState::dummy(seed))),
        };
    }

    pub fn min_y(&self) -> i32 {
        self.dimension.min_y
    }

    pub fn height(&self) -> i32 {
        self.dimension.height as i32
    }

    pub fn max_y(&self) -> i32 {
        self.min_y() + self.height() - 1
    }

    pub fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.min_y() || y > self.max_y()
    }

    pub fn set_view_distance(&mut self, view_distance: i32) {
        let view_distance = view_distance.clamp(2, 32);
        if view_distance != self.server_view_distance {
            self.server_view_distance = view_distance;
            self.distance_manager.update_player_tickets(view_distance);
        }
    }

    pub fn set_simulation_distance(&mut self, simulation_distance: i32) {
        self.distance_manager.update_simulation_distance(&mut self.holders, simulation_distance);
    }

    pub fn add_ticket(&mut self, ticket: Ticket, pos: ChunkPos) {
        self.distance_manager.add_ticket(&mut self.holders, pos.pack(), ticket);
    }

    pub fn remove_ticket(&mut self, ticket: Ticket, pos: ChunkPos) {
        self.distance_manager.remove_ticket(&mut self.holders, pos.pack(), ticket);
    }

    pub fn add_ticket_with_radius(&mut self, ty: TicketType, pos: ChunkPos, radius: i32) {
        self.distance_manager.add_ticket_with_radius(&mut self.holders, ty, pos, radius);
    }

    pub fn remove_ticket_with_radius(&mut self, ty: TicketType, pos: ChunkPos, radius: i32) {
        self.distance_manager.remove_ticket_with_radius(&mut self.holders, ty, pos, radius);
    }

    pub fn add_ticket_and_load_with_radius(&mut self, ty: TicketType, pos: ChunkPos, radius: i32) -> FutureId {
        self.add_ticket_with_radius(ty, pos, radius);
        self.run_distance_manager_updates();
        assert!(self.holders.map.contains_key(&pos.pack()), "No chunk was scheduled for loading");
        self.chunk_range_future(pos.pack(), radius, |_| Some(ChunkStatus::Full), |c| RangeThen::Complete(c.holders.futures.new_pending()))
    }

    pub fn when_complete_run_spawn_search(&mut self, future: FutureId, finder: u64) {
        self.holders.futures.then(future, Waiter::SpawnSearch(finder));
        self.process_woken();
    }

    pub fn update_chunk_forced(&mut self, pos: ChunkPos, forced: bool) -> bool {
        self.distance_manager.update_chunk_forced(&mut self.holders, pos, forced)
    }

    pub fn force_loaded_chunks(&self) -> &wafer_core::fastutil::LongOpenHashSet {
        self.distance_manager.ticket_storage.force_loaded_chunks()
    }

    pub fn activate_all_deactivated_tickets(&mut self) {
        self.distance_manager.activate_all_deactivated_tickets(&mut self.holders);
    }

    pub fn deactivate_tickets_on_closing(&mut self) {
        self.distance_manager.deactivate_tickets_on_closing(&mut self.holders);
    }

    pub fn add_player(&mut self, chunk: ChunkPos, player: crate::world::entity::EntityId) {
        self.distance_manager.add_player(&mut self.holders, chunk, player);
    }

    pub fn remove_player(&mut self, chunk: ChunkPos, player: crate::world::entity::EntityId) {
        self.distance_manager.remove_player(&mut self.holders, chunk, player);
    }

    pub fn has_active_tickets(&self) -> bool {
        self.distance_manager.ticket_storage.should_keep_dimension_active()
    }

    pub fn ticket_level(&self, pos: ChunkPos) -> Option<i32> {
        self.holders.map.get(&pos.pack()).map(|h| h.ticket_level)
    }

    pub fn main_thread_pending_tasks(&self) -> usize {
        self.distance_manager.main_thread_tasks.len() + self.holders.futures.woken.len() + self.arrived.len()
    }

    pub fn light_queue_size(&self) -> usize {
        self.light_engine.pending_task_count()
    }

    pub fn worldgen_queue_size(&self) -> usize {
        self.worldgen_queue.size()
    }

    pub fn ticket_dispatcher_queue_size(&self) -> usize {
        self.distance_manager.ticket_dispatcher_queue_size()
    }

    pub fn chunk_io_queue_size(&self) -> usize {
        self.io.as_ref().map_or(0, IoWorker::queue_size)
    }

    pub fn poi_io_queue_size(&self) -> usize {
        self.poi_manager.storage.io_queue_size()
    }

    pub fn loaded_chunks_count(&self) -> usize {
        self.holders.map.len()
    }

    fn chunk_absent(&self, key: i64, target_level: i32) -> bool {
        self.holders.map.get(&key).is_none_or(|h| h.ticket_level > target_level)
    }

    pub fn has_chunk(&self, pos: ChunkPos) -> bool {
        !self.chunk_absent(pos.pack(), chunk_level::by_status(ChunkStatus::Full))
    }

    pub fn chunks_with_at_least_full_status(&self) -> Vec<i64> {
        let level = chunk_level::by_status(ChunkStatus::Full);
        self.holders.ordered_keys().into_iter().filter(|key| self.holders.map[key].ticket_level <= level).collect()
    }

    pub fn latest_status_is_full(&self, key: i64) -> bool {
        self.holders.map.get(&key).is_some_and(|h| h.status_done(&self.holders.futures, ChunkStatus::Full))
    }

    fn is_full(&self, key: i64) -> bool {
        self.holders.map.get(&key).is_some_and(|h| !h.is_status_disallowed(ChunkStatus::Full) && h.status_done(&self.holders.futures, ChunkStatus::Full))
    }

    pub fn loaded_chunk(&self, pos: ChunkPos) -> Option<&LevelChunk> {
        if self.is_full(pos.pack()) { self.chunks.get(&pos) } else { None }
    }

    pub fn loaded_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut LevelChunk> {
        if self.is_full(pos.pack()) { self.chunks.get_mut(&pos) } else { None }
    }

    pub fn chunk(&mut self, pos: ChunkPos) -> &mut LevelChunk {
        if let Some(future) = self.begin_chunk_load(pos) {
            self.managed_block(future);
            self.end_chunk_load(pos, future);
        }
        self.chunks.get_mut(&pos).expect("full chunk")
    }

    pub fn loaded_full_chunk(&mut self, pos: ChunkPos) -> &mut LevelChunk {
        if !self.chunks.contains_key(&pos) {
            self.chunk(pos);
        }
        self.chunks.get_mut(&pos).expect("full chunk")
    }

    pub fn get_chunk_now(&self, pos: ChunkPos) -> Option<&LevelChunk> {
        profiler::increment_counter("getChunkNow", 1);
        if let Some(present) = self.cached_entry(pos) {
            return if present { self.chunks.get(&pos) } else { None };
        }
        if !self.is_full(pos.pack()) {
            return None;
        }
        self.store_in_cache(pos, ChunkStatus::Full, true);
        self.chunks.get(&pos)
    }

    pub fn count_get_chunk(&self, pos: ChunkPos) {
        profiler::increment_counter("getChunk", 1);
        if self.cached_chunk(pos, ChunkStatus::Full, true).is_none() {
            profiler::increment_counter("getChunkCacheMiss", 1);
            self.store_in_cache(pos, ChunkStatus::Full, self.is_full(pos.pack()));
        }
    }

    pub fn count_get_chunk_without_loading_at(&self, pos: ChunkPos, status: ChunkStatus) -> bool {
        profiler::increment_counter("getChunk", 1);
        match self.cached_chunk(pos, status, false) {
            Some(present) => present,
            None => {
                profiler::increment_counter("getChunkCacheMiss", 1);
                let present = self.holders.map.get(&pos.pack()).is_some_and(|h| !h.is_status_disallowed(status) && h.status_done(&self.holders.futures, status));
                self.store_in_cache(pos, status, present);
                present
            }
        }
    }

    pub fn begin_chunk_load(&mut self, pos: ChunkPos) -> Option<FutureId> {
        profiler::increment_counter("getChunk", 1);
        if self.cached_chunk(pos, ChunkStatus::Full, true).is_some() {
            return None;
        }
        profiler::increment_counter("getChunkCacheMiss", 1);
        Some(self.chunk_future_main_thread(pos, ChunkStatus::Full, true))
    }

    pub fn is_future_done(&self, future: FutureId) -> bool {
        self.holders.futures.is_done(future)
    }

    pub fn end_chunk_load(&mut self, pos: ChunkPos, future: FutureId) {
        if !self.holders.futures.is_success(future) {
            panic!("Chunk not there when requested: {pos:?}");
        }
        self.store_in_cache(pos, ChunkStatus::Full, true);
    }

    fn count_chunk_without_loading(&self, pos: ChunkPos) -> bool {
        profiler::increment_counter("getChunk", 1);
        match self.cached_chunk(pos, ChunkStatus::Full, false) {
            Some(present) => present,
            None => {
                profiler::increment_counter("getChunkCacheMiss", 1);
                let present = self.is_full(pos.pack());
                self.store_in_cache(pos, ChunkStatus::Full, present);
                present
            }
        }
    }

    pub fn chunk_without_loading(&self, pos: ChunkPos) -> Option<&LevelChunk> {
        if self.count_chunk_without_loading(pos) { self.chunks.get(&pos) } else { None }
    }

    pub fn chunk_without_loading_mut(&mut self, pos: ChunkPos) -> Option<&mut LevelChunk> {
        if self.count_chunk_without_loading(pos) { self.chunks.get_mut(&pos) } else { None }
    }

    fn cached_entry(&self, pos: ChunkPos) -> Option<bool> {
        self.last_chunk.get().into_iter().flatten().find(|&(p, status, _)| p == pos && status == ChunkStatus::Full).map(|(_, _, present)| present)
    }

    fn cached_chunk(&self, pos: ChunkPos, status: ChunkStatus, load_or_generate: bool) -> Option<bool> {
        self.last_chunk.get().into_iter().flatten().find(|&(p, s, present)| p == pos && s == status && (present || !load_or_generate)).map(|(_, _, present)| present)
    }

    fn store_in_cache(&self, pos: ChunkPos, status: ChunkStatus, present: bool) {
        let mut last_chunk = self.last_chunk.get();
        last_chunk.copy_within(0..3, 1);
        last_chunk[0] = Some((pos, status, present));
        self.last_chunk.set(last_chunk);
    }

    pub fn get_chunk_structures(&mut self, pos: ChunkPos, status: ChunkStatus) -> &ChunkStructures {
        let future = self.begin_chunk_at_status(pos, status);
        self.managed_block(future);
        self.end_chunk_at_status(pos, future)
    }

    pub fn begin_chunk_at_status(&mut self, pos: ChunkPos, status: ChunkStatus) -> FutureId {
        self.chunk_future_main_thread(pos, status, true)
    }

    pub fn end_chunk_at_status(&self, pos: ChunkPos, future: FutureId) -> &ChunkStructures {
        if !self.holders.futures.is_success(future) {
            panic!("Chunk not there when requested: {pos:?}");
        }
        self.chunk_structures(pos).expect("chunk structures")
    }

    pub fn get_generator_state(&self) -> Arc<ChunkGeneratorStructureState> {
        self.structure_state.clone().expect("generator state")
    }

    pub fn chunk_scan_access(&self) -> Option<IoWorker> {
        self.io.clone()
    }

    pub fn take_world_gen_chunk_entities(&mut self) -> Vec<(ChunkPos, Vec<wafer_protocol::nbt::Nbt>)> {
        std::mem::take(&mut self.world_gen_chunk_entities)
    }

    pub fn take_legacy_chunk_entities(&mut self) -> Vec<(ChunkPos, Vec<wafer_protocol::nbt::Nbt>)> {
        std::mem::take(&mut self.legacy_chunk_entities)
    }

    pub fn take_world_gen_entity_teams(&mut self) -> Vec<(u128, String)> {
        std::mem::take(&mut self.world_gen_entity_teams)
    }

    pub fn take_world_gen_entity_sounds(&mut self) -> Vec<(Vec3, i32, SoundSource, f32, f32)> {
        std::mem::take(&mut self.world_gen_entity_sounds)
    }

    pub fn take_structure_starts_available(&mut self) -> Vec<(ChunkPos, FastMap<StructureHolder, i32>)> {
        std::mem::take(&mut self.structure_starts_available)
    }

    fn on_structure_starts_available(&mut self, pos: ChunkPos) {
        let starts = self
            .chunk_structures(pos)
            .map(|structures| structures.starts.iter().filter(|(_, start)| start.is_valid()).map(|(&structure, start)| (structure, start.get_references())).collect())
            .unwrap_or_default();
        match &self.server_tasks {
            Some((tasks, level)) => tasks.borrow_mut().push_back(Task::StructureStarts { level: *level, pos, starts }),
            None => self.structure_starts_available.push((pos, starts)),
        }
    }

    fn chunk_future_main_thread(&mut self, pos: ChunkPos, status: ChunkStatus, load_or_generate: bool) -> FutureId {
        let key = pos.pack();
        let target_level = chunk_level::by_status(status);
        if load_or_generate {
            self.add_ticket(Ticket::new(TicketType::Unknown, target_level), pos);
            if self.chunk_absent(key, target_level) {
                profiler::push("chunkLoad");
                self.run_distance_manager_updates();
                profiler::pop();
                if self.chunk_absent(key, target_level) {
                    panic!("No chunk holder after ticket has been added: {pos:?}");
                }
            }
        }
        if self.chunk_absent(key, target_level) { UNLOADED } else { self.schedule_chunk_generation_task(key, status) }
    }

    fn next_work_result(&self) -> WorkResult {
        loop {
            match self.results.1.recv_timeout(Duration::from_millis(50)) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Timeout) => crate::crash_report::throw_delayed_exception(),
                Err(RecvTimeoutError::Disconnected) => panic!("result channel"),
            }
        }
    }

    fn managed_block(&mut self, future: FutureId) {
        let _scope = crate::server::main_thread_frames::chunk_source_block();
        while !self.holders.futures.is_done(future) {
            self.managed_block_step();
        }
    }

    pub fn managed_block_step(&mut self) {
        self.managed_block_level_step();
        self.finish_ticking_chunks_detached();
    }

    pub fn managed_block_level_step(&mut self) {
        assert!(self.run_idle_step(), "waiting for a chunk with nothing to wait for");
    }

    pub fn run_idle_step(&mut self) -> bool {
        if self.poll_task() {
            return true;
        }
        if self.in_flight.is_empty() {
            return false;
        }
        let result = self.wait_for_result();
        profiler::increment_counter("runTask", 1);
        self.apply_work_result(result);
        true
    }

    fn finish_ticking_chunks_detached(&mut self) {
        for &(_, future) in &self.ticking_ready {
            self.holders.futures.complete(future, true);
        }
    }

    fn store_arrived(&mut self, result: WorkResult) {
        if let WorkResult::Loaded { token, .. } | WorkResult::Generated { token, .. } | WorkResult::StructureStarts { token, .. } = &result {
            self.arrived.insert(*token, result);
        }
    }

    fn next_ready_result(&mut self) -> Option<WorkResult> {
        while let Ok(result) = self.results.1.try_recv() {
            self.store_arrived(result);
        }
        let front = *self.in_flight.front()?;
        self.arrived.remove(&front)
    }

    fn wait_for_result(&mut self) -> WorkResult {
        loop {
            if let Some(result) = self.next_ready_result() {
                return result;
            }
            let result = self.next_work_result();
            self.store_arrived(result);
        }
    }

    pub fn prepare_ticking_chunk(&mut self, pos: ChunkPos) {
        self.add_ticket(Ticket::new(TicketType::PlayerLoading, chunk_level::BLOCK_TICKING_LEVEL), pos);
        self.run_distance_manager_updates();
        let future = self.holders.map[&pos.pack()].ticking_chunk_future;
        self.managed_block(future);
    }

    pub fn run_until_idle(&mut self) {
        while self.run_idle_step() {
            self.finish_ticking_chunks_detached();
        }
    }

    pub fn is_ticking(&self, pos: ChunkPos) -> bool {
        self.holders.map.get(&pos.pack()).is_some_and(|h| h.is_ticking(&self.holders.futures))
    }

    pub fn chunk_to_send(&self, pos: ChunkPos) -> bool {
        self.holders.map.get(&pos.pack()).is_some_and(|h| self.holders.futures.is_done(h.send_sync) && h.is_ticking(&self.holders.futures))
    }

    pub fn wait_for_light_before_sending(&mut self, center_chunk: ChunkPos, chunk_radius: i32) {
        let affected_light_chunk_radius = chunk_radius + 1;
        for z in center_chunk.z - affected_light_chunk_radius..=center_chunk.z + affected_light_chunk_radius {
            for x in center_chunk.x - affected_light_chunk_radius..=center_chunk.x + affected_light_chunk_radius {
                let pos = ChunkPos::new(x, z);
                if self.holders.map.contains_key(&pos.pack()) {
                    let sync = self.wait_for_pending_light_tasks(pos);
                    self.add_send_dependency(pos.pack(), sync);
                }
            }
        }
    }

    fn wait_for_pending_light_tasks(&mut self, pos: ChunkPos) -> FutureId {
        let future = self.holders.futures.new_pending();
        self.add_light_task(TaskType::PostUpdate, LightTask::CompleteFuture(pos, future));
        future
    }

    fn add_send_dependency(&mut self, node: i64, sync: FutureId) {
        let current = self.holders.map[&node].send_sync;
        let combined = if self.holders.futures.is_done(current) {
            sync
        } else {
            let combined = self.holders.futures.new_pending();
            let range = self.holders.add_range(RangeWait { deps: vec![current, sync], remaining: 2, then: RangeThen::Complete(combined) });
            self.holders.futures.then(current, Waiter::Range(range));
            self.holders.futures.then(sync, Waiter::Range(range));
            combined
        };
        self.holders.map.get_mut(&node).unwrap().send_sync = combined;
    }

    pub fn send_sync_done(&self, pos: ChunkPos) -> bool {
        self.holders.map.get(&pos.pack()).is_none_or(|h| self.holders.futures.is_done(h.send_sync))
    }

    pub fn when_send_sync_done_ready_to_send(&mut self, pos: ChunkPos) {
        let Some(holder) = self.holders.map.get(&pos.pack()) else { return };
        self.holders.futures.then(holder.send_sync, Waiter::ReadyToSend(pos.pack()));
    }

    pub fn is_position_ticking(&self, pos: ChunkPos) -> bool {
        is_position_ticking(&self.distance_manager, &self.holders, pos)
    }

    pub fn take_ticking_ready(&mut self) -> Vec<(ChunkPos, FutureId)> {
        std::mem::take(&mut self.ticking_ready)
    }

    pub fn take_chunks_ready_to_send(&mut self) -> Vec<ChunkPos> {
        std::mem::take(&mut self.chunks_ready_to_send)
    }

    pub fn finish_ticking_chunk(&mut self, future: FutureId) {
        self.holders.futures.complete(future, true);
    }

    pub fn start_ticking_chunk(&mut self, pos: ChunkPos) {
        self.block_ticks.unpack_container(pos, self.game_time);
        self.fluid_ticks.unpack_container(pos, self.game_time);
    }

    pub fn pop_full_status_change(&mut self) -> Option<(ChunkPos, FullChunkStatus)> {
        self.full_status_changes.pop_front()
    }

    pub fn chunk_if_present(&mut self, pos: ChunkPos) -> Option<&mut LevelChunk> {
        self.chunks.get_mut(&pos)
    }

    pub fn collect_block_ticks(&mut self, game_time: i64, max: usize, entities_loaded: impl Fn(ChunkPos) -> bool) -> Vec<crate::world::ticks::ScheduledTick<Block>> {
        let ChunkCache { block_ticks, distance_manager, holders, .. } = self;
        block_ticks.collect(game_time, max, |c| entities_loaded(c) && is_position_ticking(distance_manager, holders, c))
    }

    pub fn collect_fluid_ticks(&mut self, game_time: i64, max: usize, entities_loaded: impl Fn(ChunkPos) -> bool) -> Vec<crate::world::ticks::ScheduledTick<Fluid>> {
        let ChunkCache { fluid_ticks, distance_manager, holders, .. } = self;
        fluid_ticks.collect(game_time, max, |c| entities_loaded(c) && is_position_ticking(distance_manager, holders, c))
    }

    pub fn pending_tasks_count(&self) -> usize {
        self.distance_manager.main_thread_tasks.len()
    }

    pub fn dump_chunks(&self, output: &mut impl std::io::Write, any_player_close_enough_for_spawning: impl Fn(ChunkPos) -> bool) -> io::Result<()> {
        let mut csv_output = crate::util::csv_output::builder()
            .add_column("x")
            .add_column("z")
            .add_column("level")
            .add_column("in_memory")
            .add_column("status")
            .add_column("full_status")
            .add_column("accessible_ready")
            .add_column("ticking_ready")
            .add_column("entity_ticking_ready")
            .add_column("ticket")
            .add_column("spawning")
            .add_column("block_entity_count")
            .add_column("ticking_ticket")
            .add_column("ticking_level")
            .add_column("block_ticks")
            .add_column("fluid_ticks")
            .build(output)?;
        let print_future = |future| match self.holders.futures.result(future) {
            Some(true) => "done",
            Some(false) => "unloaded",
            None => "not completed",
        };
        let tickets = &self.distance_manager.ticket_storage;
        for key in self.holders.ordered_keys() {
            let holder = &self.holders.map[&key];
            let chunk = holder.loaded.as_deref();
            let full_status = match holder.full_status() {
                FullChunkStatus::Inaccessible => "INACCESSIBLE",
                FullChunkStatus::Full => "FULL",
                FullChunkStatus::BlockTicking => "BLOCK_TICKING",
                FullChunkStatus::EntityTicking => "ENTITY_TICKING",
            };
            csv_output.write_row(&[
                Some(holder.pos.x.to_string()),
                Some(holder.pos.z.to_string()),
                Some(holder.ticket_level.to_string()),
                Some(chunk.is_some().to_string()),
                holder.persisted_status.map(|status| format!("minecraft:{}", status.get_name())),
                chunk.map(|_| full_status.to_string()),
                Some(print_future(holder.full_chunk_future).to_string()),
                Some(print_future(holder.ticking_chunk_future).to_string()),
                Some(print_future(holder.entity_ticking_chunk_future).to_string()),
                Some(tickets.get_ticket_debug_string(key, false)),
                Some(any_player_close_enough_for_spawning(holder.pos).to_string()),
                Some(chunk.map_or(0, |chunk| chunk.block_entities.map.len()).to_string()),
                Some(tickets.get_ticket_debug_string(key, true)),
                Some(self.distance_manager.simulation_level(key).to_string()),
                Some(chunk.map_or(0, |_| self.block_ticks.container(holder.pos).map_or(0, |ticks| ticks.count())).to_string()),
                Some(chunk.map_or(0, |_| self.fluid_ticks.container(holder.pos).map_or(0, |ticks| ticks.count())).to_string()),
            ])?;
        }
        Ok(())
    }

    pub fn block_ticking_chunks(&self) -> Vec<ChunkPos> {
        self.distance_manager.entity_ticking_chunks().into_iter().map(ChunkPos::unpack).filter(|&p| self.is_ticking(p)).collect()
    }

    pub fn purge_stale_tickets(&mut self) {
        self.distance_manager.purge_stale_tickets(&mut self.holders);
    }

    pub fn has_work(&self) -> bool {
        self.light_engine.has_work()
            || !self.holders.pending_unloads.is_empty()
            || !self.holders.map.is_empty()
            || !self.holders.to_drop.is_empty()
            || !self.unload_queue.is_empty()
            || !self.waiting_unloads.is_empty()
            || !self.in_flight.is_empty()
            || !self.holders.tasks.is_empty()
            || self.distance_manager.has_tickets()
    }

    pub fn tick_unloads(&mut self, no_save: bool, have_time: bool) {
        let deadline = self.tick_deadline;
        profiler::push("poi");
        self.poi_manager.tick(&mut || have_time || deadline.is_none_or(|deadline| Instant::now() < deadline));
        profiler::pop_push("chunk_unload");
        if !no_save {
            self.process_unloads(have_time);
        }
        profiler::pop();
        self.clear_cache();
        let count = self.holders.future_count();
        if count > 4096 && count > 2 * self.live_futures {
            self.holders.collect_garbage();
            self.live_futures = self.holders.future_count();
        }
    }

    fn clear_cache(&mut self) {
        self.last_chunk.set([None; 4]);
    }

    pub fn poll_task(&mut self) -> bool {
        let ran = self.run_one_task();
        if ran {
            profiler::increment_counter("runTask", 1);
        }
        ran
    }

    fn run_one_task(&mut self) -> bool {
        if self.process_woken() {
            return true;
        }
        if self.run_distance_manager_updates() {
            return true;
        }
        self.light_engine.try_schedule_update();
        if let Some(task) = self.distance_manager.main_thread_tasks.pop_front() {
            self.run_main_thread_task(task);
            return true;
        }
        if self.run_worldgen() {
            return true;
        }
        if self.run_light_worker() {
            return true;
        }
        if let Some(result) = self.next_ready_result() {
            self.apply_work_result(result);
            return true;
        }
        false
    }

    pub fn waker(&self) -> ResultSender {
        self.results.0.clone()
    }

    pub fn main_thread_wakeup(&self) -> Arc<MainThreadWakeup> {
        self.results.0.wakeup.clone()
    }

    pub fn set_main_thread_wakeup(&mut self, wakeup: Arc<MainThreadWakeup>) {
        self.results.0.wakeup = wakeup;
    }

    pub fn wait_for_work(&mut self, timeout: Duration) {
        if self.poll_task() {
            return;
        }
        match self.results.1.recv_timeout(timeout) {
            Ok(result) => {
                self.store_arrived(result);
                if let Some(result) = self.next_ready_result() {
                    profiler::increment_counter("runTask", 1);
                    self.apply_work_result(result);
                }
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
        }
    }

    fn process_woken(&mut self) -> bool {
        let mut any = false;
        while let Some(waiter) = self.holders.futures.woken.pop_front() {
            any = true;
            match waiter {
                Waiter::Task(task) => self.run_generation_task(task),
                Waiter::Range(range) => {
                    let wait = self.holders.ranges.get_mut(&range).expect("range");
                    wait.remaining -= 1;
                    if wait.remaining == 0 {
                        self.finish_range(range);
                    }
                }
                Waiter::ReleasePlayerTicket(pos) => self.distance_manager.main_thread_tasks.push_back(MainThreadTask::ReleasePlayerTicket(pos)),
                Waiter::ReadyToSend(node) => self.distance_manager.main_thread_tasks.push_back(MainThreadTask::ChunkReadyToSend(node)),
                Waiter::SpawnSearch(finder) => {
                    if let Some((tasks, _)) = &self.server_tasks {
                        tasks.borrow_mut().push_back(Task::SpawnSearch(finder));
                    }
                }
                Waiter::FullStatus { future, node, holder, token, status } => {
                    let confirmed = self.holders.get(node).is_some_and(|h| h.id == holder && h.pending_full_state_confirmation == token);
                    if confirmed && self.holders.futures.is_success(future) {
                        self.distance_manager.main_thread_tasks.push_back(MainThreadTask::FullChunkStatusChange { node, status });
                    }
                }
            }
        }
        any
    }

    pub fn run_distance_manager_updates(&mut self) -> bool {
        let updated = self.distance_manager.run_tracker_updates(&mut self.holders);
        let result = if !self.holders.chunks_to_update_futures.is_empty() {
            let nodes: Vec<i64> = std::mem::take(&mut self.holders.chunks_to_update_futures).into_iter().collect();
            for &node in &nodes {
                self.update_highest_allowed_status(node);
            }
            for &node in &nodes {
                self.update_futures(node);
            }
            true
        } else {
            for pos in self.distance_manager.take_tickets_to_release() {
                if self.distance_manager.has_player_loading_ticket(pos) {
                    let holder = self.holders.map.get(&pos).expect("holder of a player ticket");
                    let future = holder.entity_ticking_chunk_future;
                    self.holders.futures.then(future, Waiter::ReleasePlayerTicket(pos));
                }
            }
            updated
        };
        let promoted = std::mem::take(&mut self.holders.modified);
        self.run_generation_tasks();
        self.process_woken();
        if !result && !promoted {
            return false;
        }
        self.clear_cache();
        true
    }

    fn update_highest_allowed_status(&mut self, node: i64) {
        let Some(holder) = self.holders.map.get_mut(&node) else { return };
        let old_status = holder.highest_allowed_status;
        let new_status = chunk_level::generation_status(holder.ticket_level);
        holder.highest_allowed_status = new_status;
        let dropped = old_status.is_some_and(|old| new_status.is_none_or(|new| new < old));
        if dropped {
            self.fail_and_clear_pending_futures_between(node, new_status, old_status.unwrap());
            let holder = &self.holders.map[&node];
            if holder.task.is_some() {
                let status = self.find_highest_status_with_pending_future(node, new_status);
                self.reschedule_chunk_task(node, status);
            }
        }
    }

    fn fail_and_clear_pending_futures_between(&mut self, node: i64, from_exclusive: Option<ChunkStatus>, to_inclusive: ChunkStatus) {
        let start = from_exclusive.map_or(0, |s| s.index() + 1);
        let holder = self.holders.map.get_mut(&node).unwrap();
        for i in start..=to_inclusive.index() {
            if let Some(future) = holder.futures[i]
                && self.holders.futures.complete(future, false)
            {
                holder.futures[i] = None;
            }
        }
    }

    fn find_highest_status_with_pending_future(&self, node: i64, new_status: Option<ChunkStatus>) -> Option<ChunkStatus> {
        let holder = &self.holders.map[&node];
        let mut highest = new_status?;
        while holder.started_work.is_none_or(|started| highest > started) {
            if holder.futures[highest.index()].is_some() {
                return Some(highest);
            }
            if highest == ChunkStatus::Empty {
                break;
            }
            highest = highest.parent();
        }
        None
    }

    fn update_futures(&mut self, node: i64) {
        let holder = &self.holders.map[&node];
        let old_full_status = chunk_level::full_status(holder.old_ticket_level);
        let new_full_status = chunk_level::full_status(holder.ticket_level);
        let was_accessible = old_full_status.is_or_after(FullChunkStatus::Full);
        let is_accessible = new_full_status.is_or_after(FullChunkStatus::Full);
        self.holders.map.get_mut(&node).unwrap().was_accessible_since_last_save |= is_accessible;
        if !was_accessible && is_accessible {
            let future = self.prepare_accessible_chunk(node);
            self.holders.map.get_mut(&node).unwrap().full_chunk_future = future;
            self.schedule_full_chunk_promotion(node, future, FullChunkStatus::Full);
        }
        if was_accessible && !is_accessible {
            let holder = self.holders.map.get_mut(&node).unwrap();
            self.holders.futures.complete(holder.full_chunk_future, false);
            holder.full_chunk_future = UNLOADED;
        }
        let was_ticking = old_full_status.is_or_after(FullChunkStatus::BlockTicking);
        let is_ticking = new_full_status.is_or_after(FullChunkStatus::BlockTicking);
        if !was_ticking && is_ticking {
            let future = self.prepare_ticking_chunk_future(node);
            self.holders.map.get_mut(&node).unwrap().ticking_chunk_future = future;
            self.schedule_full_chunk_promotion(node, future, FullChunkStatus::BlockTicking);
        }
        if was_ticking && !is_ticking {
            let holder = self.holders.map.get_mut(&node).unwrap();
            self.holders.futures.complete(holder.ticking_chunk_future, false);
            holder.ticking_chunk_future = UNLOADED;
        }
        let was_entity_ticking = old_full_status.is_or_after(FullChunkStatus::EntityTicking);
        let is_entity_ticking = new_full_status.is_or_after(FullChunkStatus::EntityTicking);
        if !was_entity_ticking && is_entity_ticking {
            assert_eq!(self.holders.map[&node].entity_ticking_chunk_future, UNLOADED, "entity ticking future already there");
            let future = self.prepare_entity_ticking_chunk(node);
            self.holders.map.get_mut(&node).unwrap().entity_ticking_chunk_future = future;
            self.schedule_full_chunk_promotion(node, future, FullChunkStatus::EntityTicking);
        }
        if was_entity_ticking && !is_entity_ticking {
            let holder = self.holders.map.get_mut(&node).unwrap();
            self.holders.futures.complete(holder.entity_ticking_chunk_future, false);
            holder.entity_ticking_chunk_future = UNLOADED;
        }
        if !new_full_status.is_or_after(old_full_status) {
            self.holders.map.get_mut(&node).unwrap().pending_full_state_confirmation += 1;
            self.full_status_changes.push_back((ChunkPos::unpack(node), new_full_status));
        }
        let holder = self.holders.map.get_mut(&node).unwrap();
        self.worldgen_queue.resort_chunk_tasks(holder.queue_level, node, holder.ticket_level);
        holder.queue_level = holder.ticket_level;
        holder.old_ticket_level = holder.ticket_level;
    }

    fn schedule_full_chunk_promotion(&mut self, node: i64, future: FutureId, status: FullChunkStatus) {
        let holder = self.holders.map.get_mut(&node).unwrap();
        holder.pending_full_state_confirmation += 1;
        let waiter = Waiter::FullStatus { future, node, holder: holder.id, token: holder.pending_full_state_confirmation, status };
        self.holders.futures.then(future, waiter);
    }

    fn chunk_range_future(&mut self, center: i64, range: i32, status_at: impl Fn(i32) -> Option<ChunkStatus>, then: impl FnOnce(&mut Self) -> RangeThen) -> FutureId {
        let pos = ChunkPos::unpack(center);
        let mut deps = Vec::new();
        for z in -range..=range {
            for x in -range..=range {
                let distance = x.abs().max(z.abs());
                let node = ChunkPos::new(pos.x + x, pos.z + z).pack();
                if !self.holders.map.contains_key(&node) {
                    return UNLOADED;
                }
                let status = status_at(distance).expect("status in range");
                deps.push(self.schedule_chunk_generation_task(node, status));
            }
        }
        let then = then(self);
        let target = match then {
            RangeThen::Complete(f) | RangeThen::Ticking { future: f, .. } => f,
        };
        let range = self.holders.add_range(RangeWait { deps: deps.clone(), remaining: deps.len(), then });
        for dep in deps {
            self.holders.futures.then(dep, Waiter::Range(range));
        }
        target
    }

    fn prepare_accessible_chunk(&mut self, node: i64) -> FutureId {
        self.chunk_range_future(node, 1, chunk_level::status_around_full_chunk, |c| RangeThen::Complete(c.holders.futures.new_pending()))
    }

    fn prepare_ticking_chunk_future(&mut self, node: i64) -> FutureId {
        let pos = ChunkPos::unpack(node);
        self.chunk_range_future(node, 1, |_| Some(ChunkStatus::Full), |c| RangeThen::Ticking { pos, future: c.holders.futures.new_pending() })
    }

    fn prepare_entity_ticking_chunk(&mut self, node: i64) -> FutureId {
        self.chunk_range_future(node, 2, |_| Some(ChunkStatus::Full), |c| RangeThen::Complete(c.holders.futures.new_pending()))
    }

    fn finish_range(&mut self, range: u64) {
        if matches!(self.holders.ranges[&range].then, RangeThen::Ticking { .. }) {
            self.distance_manager.main_thread_tasks.push_back(MainThreadTask::PrepareTickingChunk(range));
            return;
        }
        let wait = self.holders.ranges.remove(&range).unwrap();
        let success = wait.deps.iter().all(|&d| self.holders.futures.is_success(d));
        if let RangeThen::Complete(future) = wait.then {
            self.holders.futures.complete(future, success);
        }
    }

    fn run_main_thread_task(&mut self, task: MainThreadTask) {
        match task {
            MainThreadTask::FullStep(token) => self.run_full_step(token),
            MainThreadTask::ApplyLoaded(token) => self.run_apply_loaded(token),
            MainThreadTask::PrepareTickingChunk(range) => self.run_prepare_ticking_chunk(range),
            MainThreadTask::FullChunkStatusChange { node, status } => self.full_status_changes.push_back((ChunkPos::unpack(node), status)),
            MainThreadTask::ChunkReadyToSend(node) => self.chunks_ready_to_send.push(ChunkPos::unpack(node)),
            MainThreadTask::LightUpdate { layer, node } => self.run_light_update_task(layer, node),
            MainThreadTask::Run { .. } | MainThreadTask::RemoveTicket(_) | MainThreadTask::Poll | MainThreadTask::ReleasePlayerTicket(_) => {
                self.distance_manager.run_main_thread_task(&mut self.holders, task)
            }
        }
    }

    fn run_prepare_ticking_chunk(&mut self, range: u64) {
        let wait = self.holders.ranges.remove(&range).unwrap();
        let success = wait.deps.iter().all(|&d| self.holders.futures.is_success(d));
        let RangeThen::Ticking { pos, future } = wait.then else { unreachable!("not a ticking range") };
        if success && self.chunks.contains_key(&pos) {
            self.ticking_ready.push((pos, future));
        } else {
            self.holders.futures.complete(future, success);
        }
    }

    fn schedule_chunk_generation_task(&mut self, node: i64, status: ChunkStatus) -> FutureId {
        let holder = &self.holders.map[&node];
        if holder.is_status_disallowed(status) {
            return UNLOADED;
        }
        let future = self.get_or_create_future(node, status);
        if self.holders.futures.is_done(future) {
            return future;
        }
        let holder = &self.holders.map[&node];
        if holder.task.is_none_or(|t| status > self.holders.tasks[&t].target_status) {
            self.reschedule_chunk_task(node, Some(status));
        }
        future
    }

    fn get_or_create_future(&mut self, node: i64, status: ChunkStatus) -> FutureId {
        let holder = self.holders.get_mut(node).unwrap();
        if holder.is_status_disallowed(status) {
            return UNLOADED;
        }
        match holder.futures[status.index()] {
            Some(future) => future,
            None => {
                let future = self.holders.futures.new_pending();
                self.holders.get_mut(node).unwrap().futures[status.index()] = Some(future);
                future
            }
        }
    }

    fn reschedule_chunk_task(&mut self, node: i64, status: Option<ChunkStatus>) {
        let new_task = status.map(|s| self.schedule_generation_task(s, ChunkPos::unpack(node)));
        let holder = self.holders.get_mut(node).unwrap();
        let old_task = std::mem::replace(&mut holder.task, new_task);
        if let Some(old) = old_task
            && let Some(task) = self.holders.tasks.get_mut(&old)
        {
            task.marked_for_cancellation = true;
        }
    }

    fn schedule_generation_task(&mut self, target_status: ChunkStatus, pos: ChunkPos) -> u64 {
        let radius = target_status.accumulated_radius_of(ChunkStatus::Empty);
        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                self.holders.get_mut(ChunkPos::new(x, z).pack()).expect("holder around a generating chunk").generation_ref_count += 1;
            }
        }
        let task = ChunkGenerationTask {
            pos,
            target_status,
            scheduled_status: None,
            marked_for_cancellation: false,
            scheduled_layer: Vec::new(),
            radius,
            needs_generation: false,
        };
        let id = self.holders.add_task(task);
        self.holders.pending_generation_tasks.push(id);
        id
    }

    fn run_generation_tasks(&mut self) {
        for task in std::mem::take(&mut self.holders.pending_generation_tasks) {
            self.run_generation_task(task);
        }
    }

    fn run_generation_task(&mut self, task: u64) {
        let Some(t) = self.holders.tasks.get(&task) else { return };
        let node = t.pos.pack();
        let level = self.holders.get(node).map_or(chunk_level::MAX_LEVEL + 1, |h| h.queue_level);
        self.worldgen_queue.submit(task, node, level.min(super::chunk_task_dispatcher::PRIORITY_LEVEL_COUNT - 1));
    }

    fn run_worldgen(&mut self) -> bool {
        let mut any = false;
        while let Some((_, tasks)) = self.worldgen_queue.pop() {
            any = true;
            for task in tasks {
                self.run_until_wait(task);
            }
            self.process_woken();
        }
        any
    }

    fn run_until_wait(&mut self, id: u64) {
        if !self.holders.tasks.contains_key(&id) {
            return;
        }
        loop {
            if let Some(waiting_for) = self.wait_for_scheduled_layer(id) {
                self.holders.futures.then(waiting_for, Waiter::Task(id));
                return;
            }
            let task = &self.holders.tasks[&id];
            if task.marked_for_cancellation || task.scheduled_status == Some(task.target_status) {
                self.release_claim(id);
                return;
            }
            self.schedule_next_layer(id);
        }
    }

    fn wait_for_scheduled_layer(&mut self, id: u64) -> Option<FutureId> {
        let ChunkHolders { tasks, futures, .. } = &mut self.holders;
        let task = tasks.get_mut(&id).unwrap();
        while let Some(&last) = task.scheduled_layer.last() {
            let Some(result) = futures.result(last) else { return Some(last) };
            task.scheduled_layer.pop();
            if !result {
                task.marked_for_cancellation = true;
            }
        }
        None
    }

    fn release_claim(&mut self, id: u64) {
        let task = self.holders.tasks.remove(&id).unwrap();
        let center = task.pos.pack();
        if let Some(holder) = self.holders.get_mut(center)
            && holder.task == Some(id)
        {
            holder.task = None;
        }
        for x in task.pos.x - task.radius..=task.pos.x + task.radius {
            for z in task.pos.z - task.radius..=task.pos.z + task.radius {
                if let Some(holder) = self.holders.get_mut(ChunkPos::new(x, z).pack()) {
                    holder.generation_ref_count -= 1;
                }
            }
        }
    }

    fn schedule_next_layer(&mut self, id: u64) {
        let task = &self.holders.tasks[&id];
        let status = match task.scheduled_status {
            None => ChunkStatus::Empty,
            Some(ChunkStatus::Empty) if !task.needs_generation && !self.can_load_without_generation(id) => {
                self.holders.tasks.get_mut(&id).unwrap().needs_generation = true;
                ChunkStatus::Empty
            }
            Some(status) => status.next(),
        };
        let needs_generation = self.holders.tasks[&id].needs_generation;
        self.schedule_layer(id, status, needs_generation);
        self.holders.tasks.get_mut(&id).unwrap().scheduled_status = Some(status);
    }

    fn can_load_without_generation(&self, id: u64) -> bool {
        let task = &self.holders.tasks[&id];
        if task.target_status == ChunkStatus::Empty {
            return true;
        }
        let persisted = |x: i32, z: i32| self.holders.get(ChunkPos::new(x, z).pack()).and_then(|h| h.persisted_status);
        if !persisted(task.pos.x, task.pos.z).is_some_and(|s| s >= task.target_status) {
            return false;
        }
        let dependencies = &ChunkPyramid::Loading.step_to(task.target_status).accumulated_dependencies;
        let range = dependencies.get_radius();
        for x in task.pos.x - range..=task.pos.x + range {
            for z in task.pos.z - range..=task.pos.z + range {
                let distance = task.pos.chessboard_distance(ChunkPos::new(x, z));
                let required = dependencies.get(distance);
                if persisted(x, z).is_none_or(|s| s < required) {
                    return false;
                }
            }
        }
        true
    }

    fn schedule_layer(&mut self, id: u64, status: ChunkStatus, needs_generation: bool) {
        let task = &self.holders.tasks[&id];
        let pyramid = if needs_generation { ChunkPyramid::Generation } else { ChunkPyramid::Loading };
        let (pos, radius) = (task.pos, pyramid.accumulated_radius_of(task.target_status, status));
        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                if self.holders.tasks[&id].marked_for_cancellation || !self.schedule_chunk_in_layer(id, status, needs_generation, ChunkPos::new(x, z)) {
                    return;
                }
            }
        }
    }

    fn schedule_chunk_in_layer(&mut self, id: u64, status: ChunkStatus, needs_generation: bool, pos: ChunkPos) -> bool {
        let persisted = self.holders.get(pos.pack()).unwrap().persisted_status;
        let generate = persisted.is_some_and(|p| status > p);
        if generate && !needs_generation {
            panic!("Can't load chunk, but didn't expect to need to generate: {pos:?}");
        }
        let future = self.apply_step(pos, status, generate);
        match self.holders.futures.result(future) {
            None => {
                self.holders.tasks.get_mut(&id).unwrap().scheduled_layer.push(future);
                true
            }
            Some(true) => true,
            Some(false) => {
                self.holders.tasks.get_mut(&id).unwrap().marked_for_cancellation = true;
                false
            }
        }
    }

    fn apply_step(&mut self, pos: ChunkPos, status: ChunkStatus, generate: bool) -> FutureId {
        let node = pos.pack();
        let holder = self.holders.get_mut(node).unwrap();
        if holder.is_status_disallowed(status) {
            return UNLOADED;
        }
        if !holder.acquire_status_bump(status) {
            return self.get_or_create_future(node, status);
        }
        let holder = holder.id;
        let work = self.holders.futures.new_pending();
        let job = Job { pos, holder, status, work };
        self.start_step(job, generate);
        work
    }

    fn new_token(&mut self, job: Job) -> u64 {
        self.next_token += 1;
        self.jobs.insert(self.next_token, job);
        self.next_token
    }

    fn new_async_token(&mut self, job: Job) -> u64 {
        let token = self.new_token(job);
        self.in_flight.push_back(token);
        token
    }

    fn start_step(&mut self, job: Job, generate: bool) {
        let pos = job.pos;
        match job.status {
            ChunkStatus::Empty => self.schedule_chunk_load(job),
            ChunkStatus::StructureStarts if generate => match (&self.generator, &self.structure_state) {
                (generator, Some(state)) if self.generate_structures => {
                    let (generator, state, dimension, results) = (generator.clone(), state.clone(), self.dimension, self.results.0.clone());
                    let upgrading = self.is_upgrading(pos);
                    let mut structures = if upgrading { self.structure_chunks.remove(&pos).unwrap_or_default() } else { ChunkStructures::default() };
                    let token = self.new_async_token(job);
                    chunk_workers::execute(move || {
                        generator.create_structures(&state, pos, dimension, &mut structures, upgrading);
                        let _ = results.send(WorkResult::StructureStarts { pos, token, structures });
                    });
                }
                _ => {
                    self.structure_chunks.insert(pos, ChunkStructures::default());
                    self.on_structure_starts_available(pos);
                    self.complete_step(job);
                }
            },
            ChunkStatus::StructureStarts => {
                self.on_structure_starts_available(pos);
                self.complete_step(job);
            }
            ChunkStatus::StructureReferences if generate => {
                self.create_references(pos);
                self.complete_step(job);
            }
            ChunkStatus::Spawn if generate => {
                self.generate_spawn(pos);
                self.complete_step(job);
            }
            ChunkStatus::Biomes if generate => {
                self.generate_biomes(pos);
                self.complete_step(job);
            }
            ChunkStatus::StructureReferences | ChunkStatus::Biomes | ChunkStatus::Spawn => self.complete_step(job),
            ChunkStatus::Terrain if generate => {
                let beardifier = self.beardifier_for_structures_in_chunk(pos).map(Arc::new);
                let blender = self.blender_for(pos);
                let biomes: [Option<Arc<crate::world::level::levelgen::chunk_generator::ChunkBiomes>>; 9] =
                    std::array::from_fn(|i| self.stored_biomes(ChunkPos::new(pos.x + i as i32 % 3 - 1, pos.z + i as i32 / 3 - 1)));
                let upgrading = self.is_upgrading(pos);
                let old = self.holders.get_mut(pos.pack()).unwrap().loaded.take();
                let token = self.new_async_token(job);
                let (generator, dimension, results) = (self.generator.clone(), self.dimension, self.results.0.clone());
                chunk_workers::execute(move || {
                    let chunk = match old {
                        Some(mut chunk) if upgrading => {
                            generator.build_terrain_upgrading(&mut chunk, dimension, beardifier, biomes, &blender);
                            *chunk
                        }
                        old => {
                            let mut chunk = generator.generate_blended(pos, dimension, beardifier, biomes, &blender);
                            if let Some(mut old) = old {
                                chunk.blending_data = old.blending_data.take();
                                chunk.carry_world_gen_data(&mut old);
                            }
                            chunk
                        }
                    };
                    let _ = results.send(WorkResult::Generated { pos, token, chunk });
                });
            }
            ChunkStatus::Terrain => {
                let holder = self.holders.get_mut(pos.pack()).unwrap();
                let decorated = holder.persisted_status.is_some_and(|status| status >= ChunkStatus::Features);
                if let Some(chunk) = holder.loaded.take() {
                    if decorated {
                        self.chunks.insert(pos, *chunk);
                    } else {
                        self.terrain_chunks.insert(pos, *chunk);
                    }
                }
                self.complete_step(job);
            }
            ChunkStatus::Features => {
                if generate {
                    self.generate_features(pos);
                }
                self.complete_step(job);
            }
            ChunkStatus::InitializeLight => {
                let lighted = self.is_lighted(pos);
                if let Some(chunk) = self.chunks.get_mut(&pos) {
                    chunk.initialize_light_sources();
                }
                let token = self.new_token(job);
                self.add_light_task(TaskType::PreUpdate, LightTask::InitializeLight(pos));
                self.add_light_task(TaskType::PostUpdate, LightTask::InitializeLightDone { pos, lighted, done: token });
            }
            ChunkStatus::Light => {
                let lighted = self.is_lighted(pos);
                if let Some(chunk) = self.chunks.get_mut(&pos) {
                    chunk.light_correct = false;
                }
                let token = self.new_token(job);
                self.add_light_task(TaskType::PreUpdate, LightTask::LightChunk { pos, lighted });
                self.add_light_task(TaskType::PostUpdate, LightTask::LightChunkDone { pos, done: token });
            }
            ChunkStatus::Full => {
                let token = self.new_token(job);
                self.distance_manager.main_thread_tasks.push_back(MainThreadTask::FullStep(token));
            }
        }
    }

    fn run_full_step(&mut self, token: u64) {
        let job = self.jobs.remove(&token).expect("full step job");
        let pos = job.pos;
        if self.holders.get(pos.pack()).is_none_or(|h| h.id != job.holder) {
            self.holders.futures.complete(job.work, false);
            return;
        }
        let mut proto_ticks = None;
        if let Some(chunk) = self.chunks.get_mut(&pos) {
            if let Some(proto) = chunk.proto.take() {
                chunk.mark_unsaved();
                let proto = *proto;
                proto_ticks = Some((LevelChunkTicks::with_pending(proto.block_ticks.scheduled_ticks()), LevelChunkTicks::with_pending(proto.fluid_ticks.scheduled_ticks())));
                if !proto.entities.is_empty() {
                    self.world_gen_chunk_entities.push((pos, proto.entities));
                }
            }
            if !chunk.legacy_entities.is_empty() {
                self.legacy_chunk_entities.push((pos, std::mem::take(&mut chunk.legacy_entities)));
            }
            let block_entities = std::mem::take(&mut chunk.post_load_block_entities);
            chunk.load_block_entities(&block_entities);
            chunk.loaded = true;
            chunk.register_all_block_entities_after_level_load(&mut self.block_entity_tickers);
            chunk.set_unsaved_listener(self.unsaved_listener.0.clone());
        }
        let holder = self.holders.get_mut(pos.pack()).unwrap();
        let (block_ticks, fluid_ticks) = match holder.saved_ticks.take() {
            Some(ticks) => *ticks,
            None => proto_ticks.unwrap_or_default(),
        };
        self.block_ticks.register_container(pos, block_ticks);
        self.fluid_ticks.register_container(pos, fluid_ticks);
        self.complete_step(job);
    }

    fn run_apply_loaded(&mut self, token: u64) {
        let job = self.jobs.remove(&token).expect("load job");
        let result = self.loaded_results.remove(&token).expect("load result");
        if self.holders.get(job.pos.pack()).is_none_or(|h| h.id != job.holder) {
            self.holders.futures.complete(job.work, false);
            return;
        }
        self.apply_loaded(job, result);
    }

    fn generate_biomes(&mut self, pos: ChunkPos) {
        let blender = self.blender_for(pos);
        let ChunkGenerator::Noise(noise) = &self.generator else { return };
        let noise = noise.clone();
        if self.is_upgrading(pos) {
            let dimension = self.dimension;
            if let Some(chunk) = self.holders.get_mut(pos.pack()).and_then(|holder| holder.loaded.as_deref_mut()) {
                noise.fill_biomes_upgrading(chunk, dimension, &blender);
            }
        } else if !blender.is_empty() {
            self.biome_chunks.insert(pos, noise.create_biomes_blended(pos, self.dimension, &blender));
        }
    }

    fn stored_biomes(&self, pos: ChunkPos) -> Option<Arc<crate::world::level::levelgen::chunk_generator::ChunkBiomes>> {
        if let Some(biomes) = self.biome_chunks.get(&pos) {
            return Some(biomes.clone());
        }
        let chunk = self.upgrading_chunk(pos)?;
        let has_biomes = self.highest_generated_status(pos).is_some_and(|status| status >= ChunkStatus::Biomes);
        has_biomes.then(|| Arc::new(crate::world::level::levelgen::chunk_generator::ChunkBiomes::from_sections(chunk.min_y(), &chunk.sections)))
    }

    fn highest_generated_status(&self, pos: ChunkPos) -> Option<ChunkStatus> {
        let persisted = self.holders.get(pos.pack()).and_then(|holder| holder.persisted_status)?;
        let target = self.upgrading_chunk(pos).and_then(|chunk| chunk.proto.as_ref()).and_then(|proto| proto.below_zero_retrogen.as_ref()).map(|retrogen| retrogen.target_status());
        Some(target.map_or(persisted, |target| target.max(persisted)))
    }

    fn is_old_noise_generation(&self, pos: ChunkPos) -> bool {
        self.upgrading_chunk(pos).is_some_and(|chunk| chunk.blending_data.is_some())
    }

    fn get_or_update_blending_data(&mut self, pos: ChunkPos) -> Option<Arc<crate::world::level::levelgen::blending::blending_data::BlendingData>> {
        use crate::world::level::levelgen::blending::blending_data::Direction8;
        if !self.is_old_noise_generation(pos) || self.highest_generated_status(pos).is_none_or(|status| status < ChunkStatus::Biomes) {
            return None;
        }
        let new_sides: Vec<Direction8> = Direction8::VALUES.into_iter().filter(|direction| !self.is_old_noise_generation(ChunkPos::new(pos.x + direction.step_x(), pos.z + direction.step_z()))).collect();
        let chunk = self.upgrading_chunk_mut(pos)?;
        let mut blending_data = chunk.blending_data.take()?;
        if !blending_data.has_calculated_data() {
            Arc::make_mut(&mut blending_data).calculate_data(chunk, &new_sides);
        }
        chunk.blending_data = Some(blending_data.clone());
        Some(blending_data)
    }

    fn upgrading_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut LevelChunk> {
        if self.terrain_chunks.contains_key(&pos) {
            return self.terrain_chunks.get_mut(&pos);
        }
        if self.chunks.contains_key(&pos) {
            return self.chunks.get_mut(&pos);
        }
        self.holders.get_mut(pos.pack()).and_then(|holder| holder.loaded.as_deref_mut())
    }

    fn blender_for(&mut self, center: ChunkPos) -> Arc<crate::world::level::levelgen::blending::blender::Blender> {
        use crate::world::level::levelgen::blending::blender::{Blender, CarvingFilter, HEIGHT_BLENDING_RANGE_CHUNKS};
        use crate::world::level::levelgen::blending::blending_data::Direction8;
        if !self.is_old_chunk_around(center, HEIGHT_BLENDING_RANGE_CHUNKS) {
            return Arc::new(Blender::empty());
        }
        let range = HEIGHT_BLENDING_RANGE_CHUNKS;
        let max_dist_sq = (range + 1) * (range + 1);
        let mut data = FastMap::default();
        for dx in -range..=range {
            for dz in -range..=range {
                if dx * dx + dz * dz <= max_dist_sq {
                    let pos = ChunkPos::new(center.x + dx, center.z + dz);
                    if let Some(blending_data) = self.get_or_update_blending_data(pos) {
                        data.insert(pos, blending_data);
                    }
                }
            }
        }
        let old_sides: Vec<(Direction8, Arc<crate::world::level::levelgen::blending::blending_data::BlendingData>)> = Direction8::VALUES
            .into_iter()
            .filter_map(|direction| {
                let side = ChunkPos::new(center.x + direction.step_x(), center.z + direction.step_z());
                self.upgrading_chunk(side).and_then(|chunk| chunk.blending_data.clone()).map(|blending_data| (direction, blending_data))
            })
            .collect();
        let center_blending_data = self.upgrading_chunk(center).and_then(|chunk| chunk.blending_data.clone());
        let carving_filter = CarvingFilter::around_old_chunks(center_blending_data.as_deref(), &old_sides);
        let blender = Blender::of(center, |x, z| data.get(&ChunkPos::new(x, z)).cloned(), carving_filter);
        Arc::new(if blender.is_empty() { Blender::empty() } else { blender })
    }

    fn is_old_chunk_around(&mut self, pos: ChunkPos, range: i32) -> bool {
        if self.io.is_none() {
            return false;
        }
        let (from_x, from_z, to_x, to_z) = (pos.x - range, pos.z - range, pos.x + range, pos.z + range);
        for region_x in from_x >> 5..=to_x >> 5 {
            for region_z in from_z >> 5..=to_z >> 5 {
                let data = self.get_or_create_old_data_for_region(region_x, region_z);
                if !data.iter().any(|&old| old) {
                    continue;
                }
                let (min_chunk_x, min_chunk_z) = (region_x << 5, region_z << 5);
                let (start_x, start_z) = ((from_x - min_chunk_x).max(0), (from_z - min_chunk_z).max(0));
                let (end_x, end_z) = ((to_x - min_chunk_x).min(31), (to_z - min_chunk_z).min(31));
                for x in start_x..=end_x {
                    for z in start_z..=end_z {
                        if data[(z * 32 + x) as usize] {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn get_or_create_old_data_for_region(&mut self, region_x: i32, region_z: i32) -> Arc<Vec<bool>> {
        let region_pos = ChunkPos::new(region_x, region_z).pack();
        if let Some(index) = self.region_cache_for_blender.get_index_of(&region_pos) {
            self.region_cache_for_blender.move_index(index, self.region_cache_for_blender.len() - 1);
            return self.region_cache_for_blender[&region_pos].clone();
        }
        let io = self.io.as_ref().unwrap();
        let mut result = vec![false; 1024];
        for local_x in 0..32 {
            for local_z in 0..32 {
                let pos = ChunkPos::new((region_x << 5) + local_x, (region_z << 5) + local_z);
                match io.scan_old_data(pos) {
                    Ok(old) => result[(local_z * 32 + local_x) as usize] = old,
                    Err(e) => {
                        let failure = crate::world::level::chunk::storage::storage_exception::scan_join_failure(&JavaThrowable::from_io_error(&e));
                        warn!(thread: chunk_workers::random_worker_name(), "Failed to scan chunk {pos}\n{}", failure.print_stack_trace().trim_end());
                    }
                }
            }
        }
        let result = Arc::new(result);
        self.region_cache_for_blender.insert(region_pos, result.clone());
        if self.region_cache_for_blender.len() > 1024 {
            self.region_cache_for_blender.shift_remove_index(0);
        }
        result
    }

    fn generate_border_ticks(&mut self, center: ChunkPos) {
        let neighbors_old = [(0, -1), (1, 0), (0, 1), (-1, 0)].map(|(dx, dz)| self.is_old_noise_generation(ChunkPos::new(center.x + dx, center.z + dz)));
        let Some(chunk) = self.upgrading_chunk_mut(center) else { return };
        let old_noise_generation = chunk.blending_data.is_some();
        let Some(blending_data) = chunk.blending_data.clone() else { return };
        let (old_min_y, old_max_y) = (blending_data.area_min_y(), blending_data.area_max_y());
        let (origin_x, origin_z) = (center.min_block_x(), center.min_block_z());
        if old_noise_generation {
            for x in 0..16 {
                for z in 0..16 {
                    for y in [old_min_y - 1, old_min_y, old_max_y, old_max_y + 1] {
                        generate_border_tick(chunk, BlockPos::new(origin_x + x, y, origin_z + z));
                    }
                }
            }
        }
        for (i, (step_x, step_z)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            if neighbors_old[i] == old_noise_generation {
                continue;
            }
            let min_x = if step_x == 1 { 15 } else { 0 };
            let max_x = if step_x == -1 { 0 } else { 15 };
            let min_z = if step_z == 1 { 15 } else { 0 };
            let max_z = if step_z == -1 { 0 } else { 15 };
            for x in min_x..=max_x {
                for z in min_z..=max_z {
                    let max_y = old_max_y.min(chunk.get_height(HeightmapType::MotionBlocking, x, z)) + 1;
                    for y in old_min_y..max_y {
                        generate_border_tick(chunk, BlockPos::new(origin_x + x, y, origin_z + z));
                    }
                }
            }
        }
    }

    fn upgrading_chunk(&self, pos: ChunkPos) -> Option<&LevelChunk> {
        if let Some(chunk) = self.terrain_chunks.get(&pos).or_else(|| self.chunks.get(&pos)) {
            return Some(chunk);
        }
        self.holders.get(pos.pack()).and_then(|holder| holder.loaded.as_deref())
    }

    fn is_upgrading(&self, pos: ChunkPos) -> bool {
        self.upgrading_chunk(pos).and_then(|chunk| chunk.proto.as_ref()).is_some_and(|proto| proto.below_zero_retrogen.is_some())
    }

    fn clear_below_zero_retrogen_on_reaching(&mut self, pos: ChunkPos, status: ChunkStatus) {
        let chunk = match self.terrain_chunks.get_mut(&pos) {
            Some(chunk) => Some(chunk),
            None => match self.chunks.get_mut(&pos) {
                Some(chunk) => Some(chunk),
                None => self.holders.get_mut(pos.pack()).and_then(|holder| holder.loaded.as_deref_mut()),
            },
        };
        if let Some(proto) = chunk.and_then(|chunk| chunk.proto.as_mut())
            && proto.below_zero_retrogen.as_ref().is_some_and(|retrogen| status >= retrogen.target_status())
        {
            proto.below_zero_retrogen = None;
        }
    }

    fn chunk_structures(&self, pos: ChunkPos) -> Option<&ChunkStructures> {
        if let Some(structures) = self.structure_chunks.get(&pos) {
            return Some(structures);
        }
        if let Some(chunk) = self.terrain_chunks.get(&pos).or_else(|| self.chunks.get(&pos)) {
            return Some(&chunk.structures);
        }
        self.holders.get(pos.pack()).and_then(|holder| holder.loaded.as_ref()).map(|chunk| &chunk.structures)
    }

    fn chunk_structures_mut(&mut self, pos: ChunkPos) -> Option<&mut ChunkStructures> {
        if self.structure_chunks.contains_key(&pos) {
            return self.structure_chunks.get_mut(&pos);
        }
        if self.terrain_chunks.contains_key(&pos) {
            return self.terrain_chunks.get_mut(&pos).map(|chunk| &mut chunk.structures);
        }
        if self.chunks.contains_key(&pos) {
            return self.chunks.get_mut(&pos).map(|chunk| &mut chunk.structures);
        }
        self.holders.get_mut(pos.pack()).and_then(|holder| holder.loaded.as_mut()).map(|chunk| &mut chunk.structures)
    }

    fn create_references(&mut self, center: ChunkPos) {
        let (target_block_x, target_block_z) = (center.min_block_x(), center.min_block_z());
        let mut references = Vec::new();
        for source_x in center.x - 8..=center.x + 8 {
            for source_z in center.z - 8..=center.z + 8 {
                let source_chunk_key = ChunkPos::new(source_x, source_z).pack();
                let Some(structures) = self.chunk_structures(ChunkPos::new(source_x, source_z)) else { continue };
                for (&structure, start) in &structures.starts {
                    if start.is_valid() && start.get_bounding_box().intersects_xz(target_block_x, target_block_z, target_block_x + 15, target_block_z + 15) {
                        references.push((structure, source_chunk_key));
                    }
                }
            }
        }
        if let Some(structures) = self.chunk_structures_mut(center) {
            for (structure, source_chunk_key) in references {
                structures.add_reference_for_structure(structure, source_chunk_key);
            }
        }
    }

    pub fn starts_for_structure(&self, pos: ChunkPos, structure: StructureHolder) -> Vec<Arc<StructureStart>> {
        let Some(references) = self.chunk_structures(pos).and_then(|structures| structures.get_references_for_structure(structure)) else { return Vec::new() };
        references
            .iter()
            .filter_map(|key| self.chunk_structures(ChunkPos::unpack(key))?.get_start_for_structure(structure).filter(|start| start.is_valid()).cloned())
            .collect()
    }

    pub fn starts_for_structure_matching(&self, pos: ChunkPos, matcher: impl Fn(StructureHolder) -> bool) -> Vec<Arc<StructureStart>> {
        let Some(structures) = self.chunk_structures(pos) else { return Vec::new() };
        structures.references.keys().filter(|&&structure| matcher(structure)).flat_map(|&structure| self.starts_for_structure(pos, structure)).collect()
    }

    fn beardifier_for_structures_in_chunk(&self, pos: ChunkPos) -> Option<Beardifier> {
        let references = self.chunk_structures(pos)?;
        let structure_starts: Vec<Arc<StructureStart>> = references
            .references
            .keys()
            .filter(|structure| structure.value().terrain_adaptation() != TerrainAdjustment::None)
            .flat_map(|&structure| self.starts_for_structure(pos, structure))
            .collect();
        Beardifier::for_structures_in_chunk(&structure_starts, pos)
    }

    fn generate_features(&mut self, center: ChunkPos) {
        self.apply_biome_decoration(center);
        self.generate_border_ticks(center);
        if let Some(chunk) = self.terrain_chunks.remove(&center) {
            self.chunks.insert(center, chunk);
        }
    }

    fn apply_biome_decoration(&mut self, center: ChunkPos) {
        let mut structure_starts = Vec::new();
        if self.generate_structures {
            structure_starts.resize_with(crate::core::data_registries::json_entries("minecraft:worldgen/structure").len(), Vec::new);
            let referenced: Vec<StructureHolder> = self.chunk_structures(center).map(|structures| structures.references.keys().copied().collect()).unwrap_or_default();
            for structure in referenced {
                structure_starts[structure.0 as usize] = self.starts_for_structure(center, structure);
            }
        }
        let structure_starts = Arc::new(structure_starts);
        self.with_region(center, |region, generator| {
            region.structure_starts = Some(structure_starts.clone());
            generator.apply_biome_decoration(region, &structure_starts);
        });
    }

    fn generate_spawn(&mut self, center: ChunkPos) {
        let spawn_mobs = self.game_rules.get_bool(wafer_data::game_rule::SPAWN_MOBS);
        self.with_region(center, |region, generator| {
            if !region.is_center_upgrading() {
                generator.spawn_original_mobs(region, spawn_mobs);
            }
        });
    }

    pub(crate) fn with_region<R>(&mut self, center: ChunkPos, f: impl FnOnce(&mut WorldGenRegion, &ChunkGenerator) -> R) -> R {
        let dimension = self.dimension;
        let generator = self.generator.clone();
        let region_random_state = self.region_random_state.clone();
        let mut taken: Vec<(ChunkPos, Option<LevelChunk>, bool)> = Vec::with_capacity(9);
        let mut writable = [false; 9];
        let mut lit = [false; 9];
        for (i, (dx, dz)) in (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).enumerate() {
            let pos = ChunkPos::new(center.x + dx, center.z + dz);
            let (chunk, terrain) = match self.terrain_chunks.remove(&pos) {
                Some(chunk) => (Some(chunk), true),
                None => (self.chunks.remove(&pos), false),
            };
            writable[i] = chunk.as_ref().is_some_and(|c| c.proto.is_some());
            lit[i] = self.holders.get(pos.pack()).is_some_and(|h| h.status_done(&self.holders.futures, ChunkStatus::InitializeLight));
            taken.push((pos, chunk, terrain));
        }
        let (result, light_tasks, poi_updates, entity_teams, entity_sounds) = {
            let mut iter = taken.iter_mut();
            let cache: [Option<&mut LevelChunk>; 9] = std::array::from_fn(|_| iter.next().and_then(|(_, c, _)| c.as_mut()));
            let generation = WorldGenerationContext {
                min_gen_y: dimension.min_y.max(generator.min_y()),
                gen_depth: (dimension.height as i32).min(generator.gen_depth()),
                sea_level: generator.sea_level(),
            };
            let random_state = match &generator {
                ChunkGenerator::Noise(noise) => &noise.state,
                ChunkGenerator::Flat(_) | ChunkGenerator::Debug(_) => region_random_state.as_deref().expect("a flat random state"),
            };
            let mut region = WorldGenRegion::new(center, cache, writable, lit, &generator, random_state, dimension, generation, &self.light_engine.engine, self.difficulty);
            region.level_data = self.region_level_data;
            region.environment_attributes = Some(&self.static_environment_attributes);
            region.scoreboard = Some(&self.scoreboard);
            let result = crate::logging::with_thread_name(super::chunk_workers::random_worker_name(), || f(&mut region, &generator));
            (
                result,
                std::mem::take(&mut region.light_tasks),
                std::mem::take(&mut region.poi_updates),
                std::mem::take(&mut region.entity_teams),
                std::mem::take(&mut region.entity_sounds),
            )
        };
        for (i, (pos, chunk, terrain)) in taken.into_iter().enumerate() {
            if let Some(mut chunk) = chunk {
                if writable[i] {
                    chunk.mark_unsaved();
                }
                if terrain {
                    self.terrain_chunks.insert(pos, chunk);
                } else {
                    self.chunks.insert(pos, chunk);
                }
            }
        }
        for task in light_tasks {
            match task {
                RegionLightTask::UpdateSectionStatus(pos, empty) => {
                    self.add_light_task(TaskType::PreUpdate, LightTask::UpdateSectionStatus(section::of(pos.x >> 4, pos.y >> 4, pos.z >> 4), empty));
                }
                RegionLightTask::CheckBlock(pos) => self.add_light_task(TaskType::PreUpdate, LightTask::CheckBlock(pos)),
            }
        }
        for (pos, old, new) in poi_updates {
            self.poi_manager.update_on_block_state_change(pos, old, new);
        }
        self.world_gen_entity_teams.extend(entity_teams);
        self.world_gen_entity_sounds.extend(entity_sounds);
        result
    }

    fn is_lighted(&self, pos: ChunkPos) -> bool {
        let persisted = self.holders.get(pos.pack()).and_then(|h| h.persisted_status);
        persisted.is_some_and(|s| s >= ChunkStatus::Light) && self.chunks.get(&pos).is_some_and(|c| c.light_correct)
    }

    fn schedule_chunk_load(&mut self, job: Job) {
        let pos = job.pos;
        if self.io.is_none() {
            self.apply_loaded(job, Ok(None));
            return;
        }
        self.poi_manager.storage.prefetch(pos);
        let token = self.new_async_token(job);
        let (dimension, seed, results, context) = (self.dimension, self.seed, self.results.0.clone(), self.data_fix_context.clone());
        self.io.as_ref().unwrap().read(pos, move |read| {
            chunk_workers::execute(move || {
                let result = crate::util::catch_exception(|| {
                    read.and_then(|nbt| match nbt {
                        None => Ok(None),
                        Some(nbt) => {
                            let nbt = simple_region_storage::upgrade_chunk_tag(nbt, -1, context.as_ref(), current_version(), DataFixTypes::Chunk);
                            match serialization::read(&nbt, dimension, seed) {
                                Ok(chunk) => Ok(Some(chunk)),
                                Err(serialization::ReadError::MissingLevelData) => {
                                    error!("Chunk file at {pos} is missing level data, skipping");
                                    Ok(None)
                                }
                                Err(serialization::ReadError::Exception(throwable)) => Err(io::Error::other(throwable)),
                            }
                        }
                    })
                })
                .unwrap_or_else(|message| Err(io::Error::other(JavaThrowable::new(message, Vec::new()))));
                let _ = results.send(WorkResult::Loaded { pos, token, result });
            });
        });
    }

    fn apply_loaded(&mut self, job: Job, result: io::Result<Option<serialization::ReadChunk>>) {
        profiler::increment_counter("chunkLoad", 1);
        let pos = job.pos;
        let result = match self.poi_manager.storage.prefetched_load_failure(pos) {
            Some(exception) => Err(io::Error::other(exception)),
            None => result,
        };
        let read = match result {
            Ok(read) => read,
            Err(e) => {
                if let Some(reporting) = &self.reporting {
                    let throwable = JavaThrowable::from_io_error(&e);
                    reporting.reporter.report_chunk_load_failure(chunk_load_failure_cause(&throwable), &reporting.info, pos);
                }
                None
            }
        };
        let persisted = match read {
            Some(serialization::ReadChunk { mut chunk, status, block_ticks, fluid_ticks, light }) => {
                if chunk.pos != pos {
                    error!("Chunk file at {pos} is in the wrong location; relocating. (Expected {pos}, got {})", chunk.pos);
                    if let Some(reporting) = &self.reporting {
                        reporting.reporter.report_misplaced_chunk(chunk.pos, pos, &reporting.info, reporting.reporter.chunk_map_load_frames());
                    }
                    chunk.pos = pos;
                }
                let min_section_y = self.dimension.min_y >> 4;
                for (i, section) in chunk.sections.iter().enumerate() {
                    self.poi_manager.check_consistency_with_blocks(pos, min_section_y + i as i32, section);
                }
                let mut loaded_any_light = false;
                for SectionLight { y, block, sky } in light {
                    let sky = sky.filter(|_| self.dimension.has_skylight);
                    if block.is_none() && sky.is_none() {
                        continue;
                    }
                    if !loaded_any_light {
                        self.add_light_task(TaskType::PreUpdate, LightTask::RetainData(pos, true));
                        loaded_any_light = true;
                    }
                    let section_node = section::of(pos.x, y, pos.z);
                    if block.is_some() {
                        self.add_light_task(TaskType::PreUpdate, LightTask::QueueSectionData(LightLayer::Block, section_node, block));
                    }
                    if sky.is_some() {
                        self.add_light_task(TaskType::PreUpdate, LightTask::QueueSectionData(LightLayer::Sky, section_node, sky));
                    }
                }
                if status < ChunkStatus::Terrain {
                    let mut chunk = chunk;
                    self.structure_chunks.insert(pos, std::mem::take(&mut chunk.structures));
                    self.loaded_structure_chunks.insert(pos);
                    if chunk.blending_data.is_some() || chunk.proto.as_ref().is_some_and(|proto| proto.below_zero_retrogen.is_some()) || chunk.has_world_gen_data() {
                        self.holders.get_mut(pos.pack()).unwrap().loaded = Some(Box::new(chunk));
                    }
                } else {
                    let holder = self.holders.get_mut(pos.pack()).unwrap();
                    holder.loaded = Some(Box::new(chunk));
                    if status == ChunkStatus::Full {
                        holder.saved_ticks = Some(Box::new((block_ticks, fluid_ticks)));
                    }
                }
                status
            }
            None => ChunkStatus::Empty,
        };
        self.holders.get_mut(pos.pack()).unwrap().persisted_status = Some(persisted);
        self.complete_step(job);
    }

    fn apply_work_result(&mut self, result: WorkResult) {
        let (token, pos) = match &result {
            WorkResult::Loaded { token, pos, .. } | WorkResult::Generated { token, pos, .. } | WorkResult::StructureStarts { token, pos, .. } => (*token, *pos),
            WorkResult::Wake => return,
        };
        self.in_flight.pop_front();
        if let WorkResult::Loaded { result, .. } = result {
            self.loaded_results.insert(token, result);
            self.distance_manager.main_thread_tasks.push_back(MainThreadTask::ApplyLoaded(token));
            return;
        }
        let job = self.jobs.remove(&token).expect("job of a result");
        if self.holders.get(pos.pack()).is_none_or(|h| h.id != job.holder) {
            self.holders.futures.complete(job.work, false);
            return;
        }
        match result {
            WorkResult::Loaded { .. } => unreachable!("loads are applied by their main thread task"),
            WorkResult::Generated { mut chunk, .. } => {
                self.biome_chunks.remove(&pos);
                chunk.mark_unsaved();
                chunk.structures = self.structure_chunks.remove(&pos).unwrap_or_default();
                self.loaded_structure_chunks.remove(&pos);
                self.terrain_chunks.insert(pos, chunk);
                self.complete_step(job);
            }
            WorkResult::StructureStarts { structures, .. } => {
                self.structure_chunks.insert(pos, structures);
                self.on_structure_starts_available(pos);
                self.complete_step(job);
            }
            WorkResult::Wake => {}
        }
        self.process_woken();
    }

    fn complete_step(&mut self, job: Job) {
        let node = job.pos.pack();
        let Some(holder) = self.holders.get_mut(node).filter(|h| h.id == job.holder) else {
            self.holders.futures.complete(job.work, false);
            return;
        };
        if job.status != ChunkStatus::Empty && holder.persisted_status.is_some_and(|p| p < job.status) {
            holder.persisted_status = Some(job.status);
        }
        if job.status != ChunkStatus::Empty {
            self.clear_below_zero_retrogen_on_reaching(job.pos, job.status);
        }
        let Some(holder) = self.holders.get_mut(node).filter(|h| h.id == job.holder) else {
            self.holders.futures.complete(job.work, false);
            return;
        };
        let index = job.status.index();
        let existing = holder.futures[index];
        let completed = match existing {
            Some(future) if self.holders.futures.complete(future, true) => true,
            Some(future) if self.holders.futures.is_success(future) => {
                panic!("Trying to complete a future but found it to be completed successfully already");
            }
            _ => false,
        };
        if !completed {
            let done = self.holders.futures.new_done(true);
            self.holders.get_mut(node).unwrap().futures[index] = Some(done);
        }
        self.holders.futures.complete(job.work, true);
    }

    fn add_light_task(&mut self, ty: TaskType, task: LightTask) {
        let queue_level = self.holders.map.get(&task.chunk_pos().pack()).map_or(super::chunk_task_dispatcher::PRIORITY_LEVEL_COUNT - 1, |holder| holder.queue_level.min(super::chunk_task_dispatcher::PRIORITY_LEVEL_COUNT - 1));
        self.light_engine.add_task(ty, task, queue_level);
    }

    fn run_light_worker(&mut self) -> bool {
        if !self.light_engine.run_worker(&mut self.chunks) {
            return false;
        }
        for token in std::mem::take(&mut self.light_engine.done) {
            let job = self.jobs.remove(&token).expect("light step");
            self.complete_step(job);
        }
        for future in std::mem::take(&mut self.light_engine.completed_futures) {
            self.holders.futures.complete(future, true);
        }
        for (layer, node) in std::mem::take(&mut self.light_engine.changes) {
            self.distance_manager.main_thread_tasks.push_back(MainThreadTask::LightUpdate { layer, node });
        }
        self.process_woken();
        true
    }

    pub fn run_light_updates(&mut self) {
        while self.light_engine.has_work() {
            self.light_engine.try_schedule_update();
            self.run_light_worker();
        }
    }

    fn run_light_update_task(&mut self, layer: LightLayer, section_node: i64) {
        let pos = ChunkPos::new(section::x(section_node), section::z(section_node));
        let Some(holder) = self.holders.map.get(&pos.pack()) else { return };
        if !holder.status_done(&self.holders.futures, ChunkStatus::InitializeLight) || holder.is_status_disallowed(ChunkStatus::InitializeLight) {
            return;
        }
        if let Some(chunk) = self.chunks.get_mut(&pos) {
            chunk.mark_unsaved();
        }
        if holder.is_ticking(&self.holders.futures) {
            self.ticking_light_changes.push((layer, section_node));
        }
    }

    pub fn take_light_changes(&mut self) -> Vec<(LightLayer, i64)> {
        std::mem::take(&mut self.ticking_light_changes)
    }

    pub fn set_block_state(&mut self, pos: BlockPos, state: BlockState) -> Option<BlockState> {
        let chunk = self.loaded_full_chunk(pos.chunk());
        let index = ((pos.y - chunk.min_y()) >> 4) as usize;
        let was_empty = chunk.sections[index].has_only_air();
        let old = chunk.set_block_state(pos, state)?;
        chunk.mark_unsaved();
        let is_empty = chunk.sections[index].has_only_air();
        let light_changed = lighting::has_different_light_properties(old, state);
        if was_empty != is_empty {
            let section_node = section::of(pos.x >> 4, pos.y >> 4, pos.z >> 4);
            self.add_light_task(TaskType::PreUpdate, LightTask::UpdateSectionStatus(section_node, is_empty));
        }
        if light_changed {
            profiler::push("updateSkyLightSources");
            if let Some(chunk) = self.chunks.get_mut(&pos.chunk()) {
                chunk.update_sky_light_sources(pos);
            }
            profiler::pop_push("queueCheckLight");
            self.add_light_task(TaskType::PreUpdate, LightTask::CheckBlock(pos));
            profiler::pop();
        }
        Some(old)
    }

    pub fn block_entity(&mut self, pos: BlockPos, creation: EntityCreationType) -> Option<&mut BlockEntity> {
        let chunk_pos = pos.chunk();
        self.loaded_full_chunk(chunk_pos);
        let ChunkCache { chunks, block_entity_tickers, .. } = self;
        chunks.get_mut(&chunk_pos).unwrap().get_block_entity(pos, creation, block_entity_tickers)
    }

    pub fn block_entity_ref(&self, pos: BlockPos) -> Option<&BlockEntity> {
        self.chunks.get(&pos.chunk())?.block_entities.map.get(&pos).filter(|e| !e.is_removed())
    }

    pub fn remove_block_entity(&mut self, pos: BlockPos) -> Option<BlockEntity> {
        self.loaded_full_chunk(pos.chunk()).remove_block_entity(pos)
    }

    pub fn add_and_register_block_entity(&mut self, entity: BlockEntity) {
        let chunk_pos = entity.pos().chunk();
        self.loaded_full_chunk(chunk_pos);
        let ChunkCache { chunks, block_entity_tickers, .. } = self;
        chunks.get_mut(&chunk_pos).unwrap().add_and_register_block_entity(entity, block_entity_tickers);
    }

    pub fn update_block_entity_ticker(&mut self, pos: BlockPos) {
        let chunk_pos = pos.chunk();
        self.loaded_full_chunk(chunk_pos);
        let ChunkCache { chunks, block_entity_tickers, .. } = self;
        chunks.get_mut(&chunk_pos).unwrap().update_block_entity_ticker(pos, block_entity_tickers);
    }

    pub fn bound_ticker(&self, id: u64, pos: BlockPos) -> Option<crate::world::level::block::entity::BlockEntityTicker> {
        self.chunks.get(&pos.chunk())?.bound_ticker(id, pos)
    }

    pub fn write_chunk_with_light(&mut self, pos: ChunkPos, w: &mut Writer) {
        if !self.chunks.contains_key(&pos) {
            self.loaded_full_chunk(pos);
        }
        let chunk = &self.chunks[&pos];
        w.i32(pos.x);
        w.i32(pos.z);
        w.layer(&[stream_codec::composite(4, 3)], |w| chunk.write_data(w));
        lighting::write_light_data(w, pos, &self.light_engine.engine, None, None);
    }

    fn process_unloads(&mut self, have_time: bool) {
        let mut dropped = Vec::new();
        self.holders.to_drop.retain(|pos| {
            dropped.push(pos);
            false
        });
        for pos in dropped {
            if let Some(holder) = self.holders.map.remove(&pos) {
                self.holders.pending_unloads.insert(pos, holder);
                self.holders.modified = true;
                self.waiting_unloads.insert(pos);
            }
        }
        let ready: Vec<i64> = self
            .waiting_unloads
            .iter()
            .copied()
            .filter(|pos| self.holders.pending_unloads.get(pos).is_none_or(|h| h.is_ready_for_saving(&self.holders.futures)))
            .collect();
        for pos in ready {
            self.waiting_unloads.shift_remove(&pos);
            self.unload_queue.push_back(pos);
        }
        let mut forced = self.unload_queue.len().saturating_sub(2000);
        while forced > 0 || have_time || self.tick_deadline.is_none_or(|deadline| Instant::now() < deadline) {
            let Some(pos) = self.unload_queue.pop_front() else { break };
            forced = forced.saturating_sub(1);
            self.unload(pos);
        }
        self.save_chunks_eagerly(have_time);
    }

    fn save_chunks_eagerly(&mut self, have_time: bool) {
        while let Ok(pos) = self.unsaved_listener.1.try_recv() {
            self.chunks_to_eagerly_save.insert(pos);
        }
        let Some(io) = &self.io else { return };
        let active_chunk_writes = io.active_chunk_writes.clone();
        let now = Instant::now();
        let mut eagerly_saved_count = 0;
        let mut index = 0;
        while eagerly_saved_count < 20
            && active_chunk_writes.load(std::sync::atomic::Ordering::Relaxed) < 128
            && (have_time || self.tick_deadline.is_none_or(|deadline| Instant::now() < deadline))
            && index < self.chunks_to_eagerly_save.len()
        {
            let pos = self.chunks_to_eagerly_save[index];
            let latest_chunk = self.holders.map.get(&pos).and_then(|holder| self.chunks.get(&holder.pos).or_else(|| self.terrain_chunks.get(&holder.pos)));
            if latest_chunk.is_none_or(|chunk| !chunk.unsaved) {
                self.chunks_to_eagerly_save.shift_remove_index(index);
            } else if self.save_chunk_if_needed(pos, now) {
                eagerly_saved_count += 1;
                self.chunks_to_eagerly_save.shift_remove_index(index);
            } else {
                index += 1;
            }
        }
    }

    fn save_chunk_if_needed(&mut self, pos: i64, now: Instant) -> bool {
        let Some(holder) = self.holders.map.get(&pos) else { return false };
        if !holder.was_accessible_since_last_save || !holder.is_ready_for_saving(&self.holders.futures) {
            return false;
        }
        let chunk_pos = holder.pos;
        if !holder.status_done(&self.holders.futures, ChunkStatus::Full) || !self.chunks.get(&chunk_pos).is_some_and(|chunk| chunk.unsaved) {
            return false;
        }
        if self.next_chunk_save_time.get(&pos).is_some_and(|&next_save_time| now < next_save_time) {
            return false;
        }
        let saved = self.save_chunk(chunk_pos);
        self.refresh_accessibility(pos);
        if saved {
            self.next_chunk_save_time.insert(pos, now + Duration::from_millis(10000));
        }
        saved
    }

    fn refresh_accessibility(&mut self, pos: i64) {
        if let Some(holder) = self.holders.map.get_mut(&pos) {
            holder.was_accessible_since_last_save = holder.full_status().is_or_after(FullChunkStatus::Full);
        }
    }

    fn unload(&mut self, pos: i64) {
        let Some(holder) = self.holders.pending_unloads.get(&pos) else { return };
        if !holder.is_ready_for_saving(&self.holders.futures) {
            self.waiting_unloads.insert(pos);
            return;
        }
        let mut holder = self.holders.pending_unloads.shift_remove(&pos).unwrap();
        let chunk_pos = holder.pos;
        self.next_chunk_save_time.remove(&pos);
        let is_full = holder.status_done(&self.holders.futures, ChunkStatus::Full);
        let terrain_chunk = self.terrain_chunks.remove(&chunk_pos);
        let structures = self.structure_chunks.remove(&chunk_pos);
        let loaded_structures = self.loaded_structure_chunks.remove(&chunk_pos);
        self.biome_chunks.remove(&chunk_pos);
        let mut chunk = self.chunks.remove(&chunk_pos);
        if let Some(chunk) = &mut chunk {
            chunk.loaded = false;
        }
        let had_chunk = chunk.is_some() || terrain_chunk.is_some() || holder.loaded.is_some() || holder.persisted_status.is_some();
        if had_chunk {
            self.poi_manager.storage.flush(chunk_pos);
        }
        if is_full && let Some(mut chunk) = chunk {
            if self.io.is_some() && chunk.unsaved {
                let block_entities = chunk.block_entities_for_saving(&mut self.block_entity_tickers);
                let light = self.light_to_save(chunk_pos);
                let block_ticks = self.block_ticks.pack(chunk_pos, self.game_time);
                let fluid_ticks = self.fluid_ticks.pack(chunk_pos, self.game_time);
                self.unloaded_block_entities.extend(chunk.block_entities.map.take_all_in_order());
                let save = ChunkToSave { chunk, status: ChunkStatus::Full, block_entities, game_time: self.game_time, block_ticks, fluid_ticks, light };
                self.io.as_ref().unwrap().write_chunk(save);
            } else {
                self.unloaded_block_entities.extend(chunk.block_entities.map.take_all_in_order());
            }
            self.block_ticks.remove_container(chunk_pos);
            self.fluid_ticks.remove_container(chunk_pos);
        } else if let Some(status) = holder.persisted_status.filter(|&status| status != ChunkStatus::Empty && status != ChunkStatus::Full) {
            let proto_chunk = if status < ChunkStatus::Terrain {
                structures.map(|structures| {
                    let mut proto_chunk = self.create_proto_chunk(chunk_pos, status, structures, loaded_structures);
                    if let Some(mut loaded) = holder.loaded.take() {
                        proto_chunk.carry_world_gen_data(&mut loaded);
                    }
                    proto_chunk
                })
            } else {
                terrain_chunk.or(chunk).or(holder.loaded.map(|chunk| *chunk))
            };
            if self.io.is_some()
                && let Some(mut proto_chunk) = proto_chunk
            {
                let block_entities = proto_chunk.block_entities_for_saving(&mut self.block_entity_tickers);
                let light = self.light_to_save(chunk_pos);
                let save = ChunkToSave { chunk: proto_chunk, status, block_entities, game_time: self.game_time, block_ticks: Vec::new(), fluid_ticks: Vec::new(), light };
                self.io.as_ref().unwrap().write_chunk(save);
            }
        }
        if had_chunk {
            self.add_light_task(TaskType::PreUpdate, LightTask::UpdateChunkStatus(chunk_pos));
            self.light_engine.try_schedule_update();
        }
    }

    fn create_proto_chunk(&self, pos: ChunkPos, status: ChunkStatus, structures: ChunkStructures, loaded: bool) -> LevelChunk {
        let mut chunk = LevelChunk::empty(pos, self.dimension, Biome::by_name("minecraft:plains").expect("plains"));
        if status >= ChunkStatus::Biomes {
            self.generator.fill_biomes_from_noise(&mut chunk, self.dimension);
        }
        chunk.proto = Some(Box::default());
        if loaded {
            for kind in HeightmapType::WORLDGEN {
                chunk.get_height(kind, pos.min_block_x(), pos.min_block_z());
            }
        }
        chunk.structures = structures;
        chunk
    }

    fn light_to_save(&self, pos: ChunkPos) -> Vec<SectionLight> {
        let engine = &self.light_engine.engine;
        let layer = |layer, y| engine.data_layer_data(layer, section::of(pos.x, y, pos.z)).filter(|l| !l.is_empty()).map(|l| l.copy());
        (engine.min_light_section()..engine.max_light_section())
            .map(|y| SectionLight { y, block: layer(LightLayer::Block, y), sky: layer(LightLayer::Sky, y) })
            .filter(|l| l.block.is_some() || l.sky.is_some())
            .collect()
    }

    fn save_chunk(&mut self, pos: ChunkPos) -> bool {
        if self.io.is_none() {
            return false;
        }
        self.poi_manager.storage.flush(pos);
        let Some(chunk) = self.chunks.get_mut(&pos).filter(|c| c.unsaved) else { return false };
        profiler::increment_counter("chunkSave", 1);
        let block_entities = chunk.block_entities_for_saving(&mut self.block_entity_tickers);
        let light = self.light_to_save(pos);
        let block_ticks = self.block_ticks.pack(pos, self.game_time);
        let fluid_ticks = self.fluid_ticks.pack(pos, self.game_time);
        match crate::util::catch_throwable(|| serialization::write(&self.chunks[&pos], ChunkStatus::Full, block_entities, self.game_time, &block_ticks, &fluid_ticks, &light)) {
            Ok(nbt) => self.io.as_ref().unwrap().write(pos, nbt),
            Err(throwable) if super::chunk_workers::is_unregistered_holder(&throwable) => super::chunk_workers::report_unregistered_holder(&throwable),
            Err(throwable) => crate::crash_report::throw_java_exception(throwable),
        }
        self.chunks.get_mut(&pos).unwrap().unsaved = false;
        true
    }

    pub fn save(&mut self, game_time: i64, flush: bool) -> io::Result<usize> {
        if self.io.is_none() {
            return Ok(0);
        }
        self.game_time = game_time;
        self.run_distance_manager_updates();
        if !flush {
            self.next_chunk_save_time.clear();
            let now = Instant::now();
            let visible = self.holders.ordered_keys();
            let saved = visible.into_iter().filter(|&pos| self.save_chunk_if_needed(pos, now)).count();
            return Ok(saved);
        }
        let chunks_to_save: Vec<i64> = self.holders.ordered_keys().into_iter().filter(|key| self.holders.map[key].was_accessible_since_last_save).collect();
        for &key in &chunks_to_save {
            self.refresh_accessibility(key);
        }
        let mut saved = 0;
        loop {
            let mut did_work = false;
            for &key in &chunks_to_save {
                while self.holders.get(key).is_some_and(|holder| !holder.is_ready_for_saving(&self.holders.futures)) {
                    self.managed_block_step();
                }
                let Some(holder) = self.holders.get(key) else { continue };
                let pos = holder.pos;
                if holder.status_done(&self.holders.futures, ChunkStatus::Full) && self.save_chunk(pos) {
                    did_work = true;
                    saved += 1;
                }
            }
            if !did_work {
                break;
            }
        }
        self.poi_manager.storage.flush_all();
        self.process_unloads(true);
        self.poi_manager.storage.synchronize(true);
        self.io.as_ref().unwrap().flush()?;
        Ok(saved)
    }
}

fn is_position_ticking(distance_manager: &DistanceManager, holders: &ChunkHolders, pos: ChunkPos) -> bool {
    let key = pos.pack();
    distance_manager.in_block_ticking_range(key) && holders.map.get(&key).is_some_and(|h| h.is_ticking(&holders.futures))
}

impl ChunkCache {
    pub fn loaded_block_state(&mut self, pos: BlockPos) -> BlockState {
        if self.is_outside_build_height(pos.y) {
            return blocks::VOID_AIR.default_state();
        }
        let is_debug = self.is_debug;
        let state = self.loaded_full_chunk(pos.chunk()).block_state(pos);
        if is_debug { debug_level_source::get_block_state(pos) } else { state }
    }
}

impl BlockGetter for ChunkCache {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        if self.is_outside_build_height(pos.y) {
            return blocks::VOID_AIR.default_state();
        }
        let is_debug = self.is_debug;
        let state = self.chunk(pos.chunk()).block_state(pos);
        if is_debug { debug_level_source::get_block_state(pos) } else { state }
    }

    fn get_min_y(&self) -> i32 {
        self.min_y()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use wafer_core::{BlockPos, ChunkPos, Vec3};
    use wafer_data::{DimensionType, blocks};

    use super::{ChunkCache, ChunkStatus, MainThreadTask, WorkResult};
    use crate::network::configuration::ClientInfo;
    use crate::network::connection::Outbound;
    use crate::network::login::Profile;
    use crate::server::OVERWORLD;
    use crate::server::level::chunk_map::ChunkTrackingView;
    use crate::server::level::server_player::ServerPlayer;
    use crate::server::level::{ServerLevel, testing};
    use crate::world::level::BlockGetter;
    use crate::world::level::chunk::FlatLevelSource;
    use crate::world::level::chunk::storage::RegionStorage;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn overworld() -> &'static DimensionType {
        &wafer_data::DIMENSION_TYPES[0]
    }

    #[test]
    fn world_gen_entity_teams_are_joined_while_the_region_runs() {
        use crate::world::level::world_gen_level::WorldGenLevel;
        let mut level = testing::level();
        level.scoreboard.borrow_mut().add_player_team("red");
        let entity = ServerLevel::create_entity("minecraft:marker").unwrap();
        let uuid = crate::world::level::storage::format_uuid(entity.entity().uuid);
        let nbt = wafer_protocol::nbt::Nbt::Compound(vec![("Team".into(), wafer_protocol::nbt::Nbt::String("red".into()))].into());
        level.chunks.with_region(ChunkPos::new(0, 0), |region, _| region.load_team(entity.entity(), &nbt));
        assert_eq!(level.scoreboard.borrow().get_players_team(&uuid).map(|team| team.name().to_string()).as_deref(), Some("red"));
    }

    #[test]
    fn world_gen_entity_teams_wait_for_a_borrowed_scoreboard() {
        use crate::world::level::world_gen_level::WorldGenLevel;
        let mut level = testing::level();
        level.scoreboard.borrow_mut().add_player_team("red");
        let entity = ServerLevel::create_entity("minecraft:marker").unwrap();
        let uuid = crate::world::level::storage::format_uuid(entity.entity().uuid);
        let nbt = wafer_protocol::nbt::Nbt::Compound(vec![("Team".into(), wafer_protocol::nbt::Nbt::String("red".into()))].into());
        let scoreboard = level.scoreboard.clone();
        let guard = scoreboard.borrow();
        level.chunks.with_region(ChunkPos::new(0, 0), |region, _| region.load_team(entity.entity(), &nbt));
        assert!(guard.get_players_team(&uuid).is_none());
        drop(guard);
        level.process_chunk_events();
        assert_eq!(level.scoreboard.borrow().get_players_team(&uuid).map(|team| team.name().to_string()).as_deref(), Some("red"));
    }

    #[test]
    fn world_gen_entity_sounds_are_queued_for_the_level() {
        use crate::world::level::world_gen_level::WorldGenLevel;
        let mut level = testing::level();
        level.chunks.with_region(ChunkPos::new(0, 0), |region, _| region.play_entity_sound(Vec3::new(1.0, 2.0, 3.0), 0, crate::server::level::SoundSource::Neutral, 1.0, 1.0));
        assert_eq!(level.chunks.take_world_gen_entity_sounds().len(), 1);
    }

    #[test]
    fn noise_chunks_are_decorated_with_ores() {
        let generator = crate::world::level::levelgen::NoiseBasedChunkGenerator::overworld(12345);
        let mut cache = ChunkCache::new(overworld(), crate::world::level::chunk::ChunkGenerator::Noise(std::sync::Arc::new(generator)), None);
        let mut ores = 0;
        for y in -64..40 {
            for z in 0..16 {
                for x in 0..16 {
                    if cache.block_state(BlockPos::new(x, y, z)).block().name().ends_with("_ore") {
                        ores += 1;
                    }
                }
            }
        }
        assert!(ores > 0);
        for cx in -4..4 {
            for cz in -4..4 {
                cache.block_state(BlockPos::new(cx * 16, 0, cz * 16));
            }
        }
        cache.run_light_updates();
    }

    #[test]
    fn decorating_next_to_a_saved_chunk_finishes() {
        let dir = std::env::temp_dir().join(format!("wafer-decorate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let generator = crate::world::level::levelgen::NoiseBasedChunkGenerator::overworld(12345);
        let chunks = ChunkCache::new(overworld(), crate::world::level::chunk::ChunkGenerator::Noise(std::sync::Arc::new(generator)), Some(RegionStorage::new(dir.clone(), false)));
        let mut level = ServerLevel::new(OVERWORLD, 0, chunks, None, 10, 10);
        let pos = BlockPos::new(100, 0, -40);
        level.block_state(pos);
        level.tick();
        level.tick();
        assert!(!level.chunks.has_chunk(pos.chunk()));
        level.block_state(BlockPos::new(84, 0, -56));
        assert!(level.chunks.has_chunk(ChunkPos::new(5, -4)));
        crate::server::level::testing::player(&mut level, wafer_core::Vec3::new(100.5, 80.0, -39.5), 0.0, 0.0);
        for _ in 0..40 {
            level.tick();
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn work_results_are_applied_in_the_order_the_work_started() {
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
        let pos = ChunkPos::new(0, 0);
        cache.in_flight.push_back(1);
        cache.in_flight.push_back(2);
        let sender = cache.waker();
        sender.send(WorkResult::Loaded { pos, token: 2, result: Ok(None) }).unwrap();
        assert!(cache.next_ready_result().is_none());
        sender.send(WorkResult::Loaded { pos, token: 1, result: Ok(None) }).unwrap();
        assert!(matches!(cache.next_ready_result(), Some(WorkResult::Loaded { token: 1, .. })));
        assert!(cache.next_ready_result().is_none());
        cache.in_flight.pop_front();
        assert!(matches!(cache.next_ready_result(), Some(WorkResult::Loaded { token: 2, .. })));
        cache.in_flight.pop_front();
        assert!(cache.next_ready_result().is_none());
    }

    #[test]
    fn a_full_chunk_read_from_disk_loads_only_the_chunks_around_it() {
        let dir = std::env::temp_dir().join(format!("wafer-loading-pyramid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
        for x in -4..=4 {
            for z in -4..=4 {
                cache.chunk(ChunkPos::new(x, z));
            }
        }
        cache.save(0, true).unwrap();
        drop(cache);
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
        cache.chunk(ChunkPos::new(0, 0));
        let persisted = |cache: &ChunkCache, x: i32, z: i32| cache.holders.get(ChunkPos::new(x, z).pack()).and_then(|holder| holder.persisted_status);
        for x in -1..=1 {
            for z in -1..=1 {
                assert_eq!(persisted(&cache, x, z), Some(ChunkStatus::Full), "{x} {z}");
            }
        }
        for (x, z) in [(2, 0), (0, 2), (-2, -2), (3, 1), (0, -4)] {
            assert_eq!(persisted(&cache, x, z), None, "{x} {z}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn flushed_saves_write_the_holders_in_their_creation_order() {
        let dir = std::env::temp_dir().join(format!("wafer-save-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
        let targets = [ChunkPos::new(5, 3), ChunkPos::new(1, 1), ChunkPos::new(3, 7), ChunkPos::new(0, 0)];
        for pos in targets {
            cache.chunk(pos);
            cache.set_block_state(BlockPos::new(pos.min_block_x(), -60, pos.min_block_z()), blocks::STONE.default_state());
        }
        let creation_order: Vec<ChunkPos> = cache.holders.ordered_keys().into_iter().map(ChunkPos::unpack).filter(|pos| targets.contains(pos)).collect();
        cache.save(0, true).unwrap();
        let header = std::fs::read(dir.join("r.0.0.mca")).unwrap();
        let mut by_sector: Vec<(u32, ChunkPos)> = targets.iter().map(|&pos| (u32::from_be_bytes(header[(pos.x + pos.z * 32) as usize * 4..][..4].try_into().unwrap()) >> 8, pos)).collect();
        by_sector.sort();
        assert_eq!(by_sector.into_iter().map(|(_, pos)| pos).collect::<Vec<_>>(), creation_order);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn structure_starts_are_queued_on_the_server_task_queue() {
        let tasks: crate::server::ServerTasks = Default::default();
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
        cache.server_tasks = Some((tasks.clone(), 2));
        cache.chunk(ChunkPos::new(0, 0));
        assert!(tasks.borrow().iter().any(|task| matches!(task, crate::server::Task::StructureStarts { level: 2, pos, .. } if *pos == ChunkPos::new(0, 0))));
        assert!(cache.take_structure_starts_available().is_empty());
    }

    #[test]
    fn the_full_step_waits_for_the_main_thread_task_queue() {
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
        let future = cache.begin_chunk_load(ChunkPos::new(2, 2)).unwrap();
        while cache.distance_manager.main_thread_tasks.iter().all(|task| !matches!(task, MainThreadTask::FullStep(_))) {
            assert!(cache.run_idle_step());
        }
        assert!(!cache.is_future_done(future));
        assert!(cache.loaded_chunk(ChunkPos::new(2, 2)).is_none());
        while !cache.is_future_done(future) {
            assert!(cache.run_idle_step());
        }
        assert!(cache.loaded_chunk(ChunkPos::new(2, 2)).is_some());
    }

    #[test]
    fn chunks_become_ticking_after_the_level_handled_them() {
        let mut level = testing::level();
        level.chunks.add_ticket_with_radius(super::TicketType::Forced, ChunkPos::new(0, 0), 1);
        let pos = ChunkPos::new(0, 0);
        let mut seen_unhandled = false;
        while level.chunks.run_idle_step() {
            if !level.chunks.ticking_ready.is_empty() {
                assert!(!level.chunks.is_ticking(pos));
                seen_unhandled = true;
                level.process_chunk_events();
                assert!(level.chunks.is_ticking(pos));
            }
        }
        assert!(seen_unhandled);
    }

    #[test]
    fn a_chunk_needed_now_loads_on_the_spot() {
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
        let pos = ChunkPos::new(5, -3);
        assert!(!cache.has_chunk(pos));
        assert_eq!(cache.block_state(BlockPos::new(80, -61, -40)), blocks::GRASS_BLOCK.default_state());
        assert!(cache.has_chunk(pos));
        assert!(cache.loaded_chunk(pos).is_some());
        assert_eq!(cache.ticket_level(pos), Some(33));
        assert_eq!(cache.ticket_level(ChunkPos::new(6, -3)), Some(34));
        assert!(!cache.has_chunk(ChunkPos::new(6, -3)));
        assert!(cache.chunk_if_present(ChunkPos::new(6, -3)).is_some());
        assert!(cache.chunk_if_present(ChunkPos::new(7, -3)).is_none());
        assert_eq!(cache.ticket_level(ChunkPos::new(16, -3)), Some(44));
        assert_eq!(cache.ticket_level(ChunkPos::new(17, -3)), None);
    }

    #[test]
    fn chunks_looked_up_without_loading_are_cached_without_tickets() {
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
        let pos = ChunkPos::new(5, -3);
        let has_unknown_ticket = |cache: &ChunkCache| cache.distance_manager.ticket_storage.tickets(pos.pack()).iter().any(|t| t.ty == super::TicketType::Unknown);
        cache.update_chunk_forced(pos, true);
        cache.block_state(BlockPos::new(80, -61, -40));
        assert!(has_unknown_ticket(&cache));
        cache.purge_stale_tickets();
        cache.purge_stale_tickets();
        assert!(!has_unknown_ticket(&cache));
        cache.clear_cache();
        assert!(cache.chunk_without_loading(pos).is_some());
        assert!(cache.chunk_without_loading(ChunkPos::new(40, 40)).is_none());
        assert_eq!(cache.block_state(BlockPos::new(80, -61, -40)), blocks::GRASS_BLOCK.default_state());
        assert!(!has_unknown_ticket(&cache));
        cache.clear_cache();
        cache.block_state(BlockPos::new(80, -61, -40));
        assert!(has_unknown_ticket(&cache));
    }

    #[test]
    fn chunks_whose_data_fixers_throw_are_regenerated() {
        let dir = std::env::temp_dir().join(format!("wafer-failing-chunk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut storage = RegionStorage::new(dir.clone(), false);
        let tag = crate::nbt::parse_fully(
            r#"{DataVersion:1495,Level:{xPos:0,zPos:0,Sections:[{Y:0b,Palette:[{Name:"minecraft:air"}]},{Y:0b,Palette:[{Name:"minecraft:stone"}]}]}}"#,
        )
        .unwrap();
        storage.write(ChunkPos::new(0, 0), &tag).unwrap();
        storage.flush().unwrap();
        let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(storage));
        assert_eq!(cache.block_state(BlockPos::new(1, -61, 1)), blocks::GRASS_BLOCK.default_state());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn expired_chunks_are_saved_unloaded_and_read_back() {
        let dir = std::env::temp_dir().join(format!("wafer-unload-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chunks = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
        let mut level = ServerLevel::new(OVERWORLD, 0, chunks, None, 10, 10);
        let pos = BlockPos::new(100, -60, 100);
        level.set_block(pos, blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.chunks.has_chunk(pos.chunk()));
        level.finish_chunk_work();
        level.tick();
        assert!(level.chunks.has_chunk(pos.chunk()));
        level.tick();
        assert!(!level.chunks.has_chunk(pos.chunk()));
        assert_eq!(level.chunks.loaded_chunks_count(), 0);
        assert!(level.chunks.chunk_if_present(pos.chunk()).is_none());
        assert_eq!(level.block_state(pos), blocks::STONE.default_state());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn chunks_reach_a_joining_player_over_time() {
        let mut level = testing::level();
        let profile = Profile { uuid: 7, name: "Joiner".into(), properties: Vec::new() };
        let mut player = ServerPlayer::new(profile, ClientInfo::default(), Outbound::detached(), BlockPos::ZERO);
        player.entity.set_pos(Vec3::new(8.5, -60.0, 8.5));
        player.requested_view_distance = 10;
        let id = player.entity.id;
        level.add_player(player);
        assert!(level.chunks.chunk_if_present(ChunkPos::new(0, 0)).is_none());
        let view = level.player(id).chunk_tracking_view.unwrap();
        let mut in_view = Vec::new();
        ChunkTrackingView::difference(None, Some(view), |p| in_view.push(p), |_| {});
        let mut ready: Vec<ChunkPos> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(120);
        while ready.len() < in_view.len() {
            assert!(Instant::now() < deadline, "only {} chunks ready", ready.len());
            if !level.poll_chunk_task() {
                level.wait_for_chunk_work(Duration::from_millis(5));
            }
            for &pos in &in_view {
                if level.player(id).chunk_sender.is_pending(pos) && !ready.contains(&pos) {
                    ready.push(pos);
                }
            }
        }
        let own = ready.iter().position(|&p| p == ChunkPos::new(0, 0)).unwrap();
        let first_far = ready.iter().position(|p| p.chessboard_distance(ChunkPos::new(0, 0)) >= 6).unwrap();
        assert!(own < first_far, "own chunk {own}, first far chunk {first_far}");
        assert!(in_view.iter().all(|&p| level.chunks.is_ticking(p)));
        assert!(!level.player(id).chunk_sender.is_pending(ChunkPos::new(view.view_distance + 3, 0)));
    }

    #[test]
    fn moving_players_move_the_ticking_chunks() {
        let mut level = testing::level();
        level.set_simulation_distance(3);
        let id = testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
        let ticking = |level: &ServerLevel| level.chunks.block_ticking_chunks();
        assert_eq!(ticking(&level).len(), 49);
        assert!(level.in_entity_ticking_range(ChunkPos::new(-3, 3)));
        level.player_mut(id).entity.set_pos(Vec3::new(5.0 * 16.0 + 8.5, -60.0, 8.5));
        level.move_player(id);
        level.finish_chunk_work();
        level.tick();
        assert!(!level.in_entity_ticking_range(ChunkPos::new(-3, 3)));
        assert!(level.in_entity_ticking_range(ChunkPos::new(8, 3)));
        let now = ticking(&level);
        assert_eq!(now.len(), 49);
        assert!(now.iter().all(|p| (2..=8).contains(&p.x) && (-3..=3).contains(&p.z)));
        let order: Vec<ChunkPos> = level.chunks.distance_manager.entity_ticking_chunks().into_iter().map(ChunkPos::unpack).collect();
        assert_eq!(now, order);
    }
}

fn generate_border_tick(chunk: &mut LevelChunk, pos: BlockPos) {
    let state = chunk.block_state(pos);
    if state.is(&wafer_data::block_tags::LEAVES) {
        chunk.mark_pos_for_post_processing(pos);
    }
    if !state.fluid_state().is_empty() {
        chunk.mark_pos_for_post_processing(pos);
    }
}
