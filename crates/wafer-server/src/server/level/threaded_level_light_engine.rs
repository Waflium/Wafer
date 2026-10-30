use wafer_core::hash_map::FastMap;

use wafer_core::{BlockPos, ChunkPos};

use super::chunk_holder::FutureId;
use super::chunk_task_dispatcher::ChunkTaskPriorityQueue;
use crate::world::level::chunk::LevelChunk;
use crate::world::level::chunk::data_layer::DataLayer;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::lighting::{LevelLightEngine, section};

const TASKS_PER_BATCH: usize = 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskType {
    PreUpdate,
    PostUpdate,
}

pub enum LightTask {
    CheckBlock(BlockPos),
    UpdateSectionStatus(i64, bool),
    QueueSectionData(LightLayer, i64, Option<DataLayer>),
    RetainData(ChunkPos, bool),
    InitializeLight(ChunkPos),
    InitializeLightDone { pos: ChunkPos, lighted: bool, done: u64 },
    LightChunk { pos: ChunkPos, lighted: bool },
    LightChunkDone { pos: ChunkPos, done: u64 },
    UpdateChunkStatus(ChunkPos),
    CompleteFuture(ChunkPos, FutureId),
}

impl LightTask {
    pub fn chunk_pos(&self) -> ChunkPos {
        match *self {
            LightTask::CheckBlock(pos) => pos.chunk(),
            LightTask::UpdateSectionStatus(section_node, _) | LightTask::QueueSectionData(_, section_node, _) => ChunkPos::new(section::x(section_node), section::z(section_node)),
            LightTask::RetainData(pos, _)
            | LightTask::InitializeLight(pos)
            | LightTask::InitializeLightDone { pos, .. }
            | LightTask::LightChunk { pos, .. }
            | LightTask::LightChunkDone { pos, .. }
            | LightTask::UpdateChunkStatus(pos)
            | LightTask::CompleteFuture(pos, _) => pos,
        }
    }

    pub fn at_top_priority(&self) -> bool {
        matches!(self, LightTask::UpdateSectionStatus(..) | LightTask::QueueSectionData(..) | LightTask::RetainData(..) | LightTask::UpdateChunkStatus(..))
    }
}

pub struct ThreadedLevelLightEngine {
    pub engine: LevelLightEngine,
    queue: ChunkTaskPriorityQueue<(TaskType, LightTask)>,
    light_tasks: Vec<(TaskType, LightTask)>,
    scheduled: bool,
    pub changes: Vec<(LightLayer, i64)>,
    pub done: Vec<u64>,
    pub completed_futures: Vec<FutureId>,
    min_section: i32,
    max_section: i32,
}

impl ThreadedLevelLightEngine {
    pub fn new(min_y: i32, height: i32, has_sky_light: bool) -> ThreadedLevelLightEngine {
        ThreadedLevelLightEngine {
            engine: LevelLightEngine::new(min_y, height, true, has_sky_light),
            queue: ChunkTaskPriorityQueue::default(),
            light_tasks: Vec::new(),
            scheduled: false,
            changes: Vec::new(),
            done: Vec::new(),
            completed_futures: Vec::new(),
            min_section: min_y >> 4,
            max_section: ((min_y + height) >> 4) - 1,
        }
    }

    pub fn add_task(&mut self, ty: TaskType, task: LightTask, queue_level: i32) {
        let level = if task.at_top_priority() { 0 } else { queue_level };
        let pos = task.chunk_pos().pack();
        self.queue.submit((ty, task), pos, level);
    }

    pub fn has_work(&self) -> bool {
        self.queue.has_work() || !self.light_tasks.is_empty() || self.engine.has_light_work()
    }

    pub fn try_schedule_update(&mut self) {
        if self.has_work() {
            self.scheduled = true;
        }
    }

    pub fn run_worker(&mut self, chunks: &mut FastMap<ChunkPos, LevelChunk>) -> bool {
        let mut worked = false;
        while let Some((_, tasks)) = self.queue.pop() {
            worked = true;
            for task in tasks {
                self.light_tasks.push(task);
                if self.light_tasks.len() >= TASKS_PER_BATCH {
                    self.run_update(chunks);
                }
            }
        }
        if self.scheduled {
            self.scheduled = false;
            worked = true;
            self.run_update(chunks);
        }
        worked
    }

    fn run_update(&mut self, chunks: &mut FastMap<ChunkPos, LevelChunk>) {
        let total = self.light_tasks.len().min(TASKS_PER_BATCH);
        let mut batch: Vec<(TaskType, LightTask)> = self.light_tasks.drain(..total).collect();
        let mut correct = Vec::new();
        {
            let chunks: &FastMap<ChunkPos, LevelChunk> = chunks;
            for (ty, task) in &mut batch {
                if *ty == TaskType::PreUpdate {
                    self.run_task(chunks, task, &mut correct);
                }
            }
            let mut changes = std::mem::take(&mut self.changes);
            self.engine.run_light_updates(chunks, &mut changes);
            self.changes = changes;
            for (ty, task) in &mut batch {
                if *ty == TaskType::PostUpdate {
                    self.run_task(chunks, task, &mut correct);
                }
            }
        }
        for pos in correct {
            if let Some(chunk) = chunks.get_mut(&pos) {
                chunk.light_correct = true;
            }
        }
    }

    fn run_task(&mut self, chunks: &FastMap<ChunkPos, LevelChunk>, task: &mut LightTask, correct: &mut Vec<ChunkPos>) {
        let engine = &mut self.engine;
        if let LightTask::QueueSectionData(layer, section_node, data) = task {
            engine.queue_section_data(*layer, *section_node, data.take());
            return;
        }
        match *task {
            LightTask::CheckBlock(pos) => engine.check_block(pos),
            LightTask::UpdateSectionStatus(section_node, empty) => engine.update_section_status(section_node, empty),
            LightTask::QueueSectionData(..) => {}
            LightTask::RetainData(pos, retain) => engine.retain_data(pos, retain),
            LightTask::InitializeLight(pos) => {
                let Some(chunk) = chunks.get(&pos) else { return };
                let min_section_y = chunk.min_y() >> 4;
                for (index, s) in chunk.sections.iter().enumerate() {
                    if !s.has_only_air() {
                        engine.update_section_status(section::of(pos.x, min_section_y + index as i32, pos.z), false);
                    }
                }
            }
            LightTask::InitializeLightDone { pos, lighted, done: future } => {
                engine.set_light_enabled(chunks, pos, lighted);
                engine.retain_data(pos, false);
                self.done.push(future);
            }
            LightTask::LightChunk { pos, lighted } => {
                if !lighted {
                    engine.propagate_light_sources(chunks, pos);
                }
            }
            LightTask::LightChunkDone { pos, done: future } => {
                correct.push(pos);
                self.done.push(future);
            }
            LightTask::UpdateChunkStatus(pos) => {
                engine.retain_data(pos, false);
                engine.set_light_enabled(chunks, pos, false);
                for section_y in engine.min_light_section()..engine.max_light_section() {
                    engine.queue_section_data(LightLayer::Block, section::of(pos.x, section_y, pos.z), None);
                    engine.queue_section_data(LightLayer::Sky, section::of(pos.x, section_y, pos.z), None);
                }
                for section_y in self.min_section..=self.max_section {
                    engine.update_section_status(section::of(pos.x, section_y, pos.z), true);
                }
            }
            LightTask::CompleteFuture(_, future) => self.completed_futures.push(future),
        }
    }
}
