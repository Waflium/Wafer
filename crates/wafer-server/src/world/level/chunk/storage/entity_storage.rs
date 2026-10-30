use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use wafer_core::ChunkPos;
use wafer_core::hash_map::FastSet;
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::RegionStorage;
use super::storage_exception;
use crate::server::level::ServerLevel;
use crate::server::level::chunk_workers::{IoReporting, IoWorker, MainThreadWakeup};
use crate::util::java_throwable::JavaThrowable;
use crate::util::problem_reporter::PathElement;
use crate::world::entity::LevelEntity;
use crate::world::level::storage::tag_value_input;

pub const ENTITY_STORAGE_LOGGER: &str = "net.minecraft.world.level.chunk.storage.EntityStorage";

const LOAD_ENTITIES_RECURSIVE: [&str; 18] = [
    "net.minecraft.world.entity.EntityType.loadEntityRecursive(EntityType.java:358)",
    "net.minecraft.world.entity.EntityType.lambda$loadEntitiesRecursive$0(EntityType.java:388)",
    "java.base/java.util.stream.ReferencePipeline$11$1.accept(ReferencePipeline.java:489)",
    "java.base/java.util.stream.ReferencePipeline$2$1.accept(ReferencePipeline.java:197)",
    "com.google.common.collect.Streams$1Splitr.tryAdvance(Streams.java:489)",
    "java.base/java.util.Spliterator.forEachRemaining(Spliterator.java:332)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:635)",
    "java.base/java.util.stream.AbstractPipeline.evaluateToArrayNode(AbstractPipeline.java:291)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:652)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:658)",
    "java.base/java.util.stream.ReferencePipeline.toList(ReferencePipeline.java:663)",
    "net.minecraft.world.level.chunk.storage.EntityStorage.lambda$loadEntities$0(EntityStorage.java:82)",
    "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:667)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.run(CompletableFuture.java:503)",
    "net.minecraft.util.Util.runNamed(Util.java:351)",
    "net.minecraft.util.thread.AbstractConsecutiveExecutor.pollTask(AbstractConsecutiveExecutor.java:50)",
];

const ENTITY_DESERIALIZER_FRAMES: [&str; 4] = [
    "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:667)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.run(CompletableFuture.java:503)",
    "net.minecraft.util.Util.runNamed(Util.java:351)",
    "net.minecraft.util.thread.AbstractConsecutiveExecutor.pollTask(AbstractConsecutiveExecutor.java:50)",
];

fn entity_storage_frames(first: &'static str) -> Vec<&'static str> {
    let mut frames = vec![first];
    frames.extend(ENTITY_DESERIALIZER_FRAMES);
    frames.extend(crate::server::main_thread_frames::current());
    frames
}

pub type EntityChunkRead = (ChunkPos, io::Result<Option<Nbt>>);

pub struct EntityStorage {
    worker: Option<IoWorker>,
    empty_chunks: FastSet<i64>,
    reads: (Sender<EntityChunkRead>, Receiver<EntityChunkRead>),
    loading_inbox: Arc<AtomicUsize>,
    reporting: Option<IoReporting>,
    main_thread_wakeup: Arc<MainThreadWakeup>,
}

impl EntityStorage {
    pub fn new(storage: Option<RegionStorage>) -> EntityStorage {
        EntityStorage {
            worker: storage.map(IoWorker::new),
            empty_chunks: FastSet::default(),
            reads: channel(),
            loading_inbox: Arc::default(),
            reporting: None,
            main_thread_wakeup: Arc::default(),
        }
    }

    pub fn set_main_thread_wakeup(&mut self, wakeup: Arc<MainThreadWakeup>) {
        self.main_thread_wakeup = wakeup;
    }

    pub fn reporting(&self) -> Option<&IoReporting> {
        self.reporting.as_ref()
    }

    pub fn set_reporting(&mut self, reporting: IoReporting) {
        if let Some(worker) = &self.worker {
            worker.set_reporting(reporting.clone());
        }
        self.reporting = Some(reporting);
    }

    pub fn load_entities(&mut self, pos: ChunkPos) {
        let Some(worker) = self.worker.as_ref().filter(|_| !self.empty_chunks.contains(&pos.pack())) else {
            self.loading_inbox.fetch_add(1, Ordering::Relaxed);
            let _ = self.reads.0.send((pos, Ok(None)));
            return;
        };
        let reads = self.reads.0.clone();
        let loading_inbox = self.loading_inbox.clone();
        let reporting = self.reporting.clone();
        let wakeup = self.main_thread_wakeup.clone();
        worker.read(pos, move |read| {
            if let Err(e) = &read {
                let throwable = JavaThrowable::from_io_error(e);
                error!("Failed to read chunk {pos}\n{}", storage_exception::entity_manager_completion_exception(&throwable).print_stack_trace().trim_end());
                error!("Failed to load entity chunk {pos}\n{}", throwable.print_stack_trace().trim_end());
                if let Some(reporting) = &reporting {
                    reporting.reporter.report_chunk_load_failure(&throwable, &reporting.info, pos);
                }
            }
            loading_inbox.fetch_add(1, Ordering::Relaxed);
            drop(reads.send((pos, read)));
            wakeup.wake();
        });
    }

    pub fn poll_read(&mut self) -> Option<EntityChunkRead> {
        let read = self.reads.1.try_recv().ok()?;
        self.loading_inbox.fetch_sub(1, Ordering::Relaxed);
        Some(read)
    }

    pub fn loading_inbox_size(&self) -> usize {
        self.loading_inbox.load(Ordering::Relaxed)
    }

    pub fn mark_empty(&mut self, pos: ChunkPos) {
        self.empty_chunks.insert(pos.pack());
    }

    pub fn store_entities(&mut self, pos: ChunkPos, entities: Option<Vec<Nbt>>) {
        match entities {
            None => {
                if self.empty_chunks.insert(pos.pack())
                    && let Some(worker) = &self.worker
                {
                    worker.clear(pos);
                }
            }
            Some(entities) => {
                if let Some(worker) = &self.worker {
                    worker.write(pos, chunk_tag(pos, entities));
                }
                self.empty_chunks.remove(&pos.pack());
            }
        }
    }

    pub fn flush(&self, flush_storage: bool) {
        if let Some(worker) = &self.worker
            && let Err(e) = worker.synchronize(flush_storage)
        {
            warn!("Failed to synchronize entity chunks: {e}");
        }
    }
}

fn stored_position(tag: &Nbt) -> Result<ChunkPos, Option<&'static str>> {
    match tag.get("Position") {
        None => Err(None),
        Some(Nbt::IntArray(stored)) if stored.len() == 2 => Ok(ChunkPos::new(stored[0], stored[1])),
        Some(Nbt::List(elements)) if elements.len() == 2 && elements.iter().all(|element| matches!(element, Nbt::Int(_))) => {
            Ok(ChunkPos::new(elements[0].as_i64().unwrap_or_default() as i32, elements[1].as_i64().unwrap_or_default() as i32))
        }
        Some(Nbt::IntArray(_)) => Err(Some("Input is not a list of 2 ints")),
        Some(Nbt::List(elements)) if elements.iter().all(|element| matches!(element, Nbt::Int(_))) => Err(Some("Input is not a list of 2 ints")),
        Some(_) => Err(Some("Not a list")),
    }
}

pub fn chunk_tag(pos: ChunkPos, entities: Vec<Nbt>) -> Nbt {
    Nbt::Compound(vec![
        ("DataVersion".into(), Nbt::Int(WORLD_VERSION)),
        ("Entities".into(), Nbt::List(entities)),
        ("Position".into(), Nbt::IntArray(vec![pos.x, pos.z])),
    ].into())
}

impl ServerLevel {
    pub fn read_chunk_entities(&mut self, pos: ChunkPos, tag: &Nbt) -> Vec<LevelEntity> {
        let reporting = self.entity_manager.reporting().cloned();
        match stored_position(tag) {
            Ok(stored) => {
                if stored != pos {
                    error!("Chunk file at {pos} is in the wrong location. (Expected {pos}, got {stored})");
                    if let Some(reporting) = &reporting {
                        reporting.reporter.report_misplaced_chunk(stored, pos, &reporting.info, entity_storage_frames(storage_exception::ENTITY_STORAGE_MISPLACED_FRAME));
                    }
                }
            }
            Err(field_error) => {
                if let (Some(field_error), Some(position)) = (field_error, tag.get("Position")) {
                    error!("Failed to read field (Position={}): {field_error}", crate::nbt::to_string(position));
                }
                let mut frames = vec![storage_exception::OPTIONAL_OR_ELSE_THROW];
                frames.extend(entity_storage_frames(storage_exception::ENTITY_STORAGE_POSITION_FRAME));
                let throwable = JavaThrowable::new("java.util.NoSuchElementException: No value present", frames);
                warn!("Failed to parse chunk {pos} position info\n{}", throwable.print_stack_trace().trim_end());
                if let Some(reporting) = &reporting {
                    reporting.reporter.report_chunk_load_failure(&throwable, &reporting.info, pos);
                }
            }
        }
        let upgraded_chunk_tag = super::simple_region_storage::upgrade_chunk_tag_to_current_version(tag, -1, crate::util::datafix::data_fix_types::DataFixTypes::EntityChunk);
        let tag = &*upgraded_chunk_tag;
        let _collector = tag_value_input::ScopedCollector::new(PathElement::Text(format!("chunk@[{}, {}]", pos.x, pos.z)), ENTITY_STORAGE_LOGGER);
        let mut site = LOAD_ENTITIES_RECURSIVE.to_vec();
        site.extend(crate::server::main_thread_frames::current());
        let mut loaded = Vec::new();
        tag_value_input::for_each_child(tag, "Entities", |entity| loaded.extend(self.load_entity_recursive_in_scope(entity, &site, &mut |_| {})));
        loaded
    }
}
