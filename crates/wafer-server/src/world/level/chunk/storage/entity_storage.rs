use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use wafer_core::ChunkPos;
use wafer_core::hash_map::FastSet;
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::RegionStorage;
use crate::server::level::ServerLevel;
use crate::server::command_frames::main_thread_task_frames;
use crate::server::level::chunk_workers::IoWorker;
use crate::util::problem_reporter::PathElement;
use crate::world::entity::LevelEntity;
use crate::world::level::storage::tag_value_input;

pub const ENTITY_STORAGE_LOGGER: &str = "net.minecraft.world.level.chunk.storage.EntityStorage";

const LOAD_ENTITIES_RECURSIVE: [&str; 19] = [
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
    "net.minecraft.util.thread.AbstractConsecutiveExecutor.run(AbstractConsecutiveExecutor.java:60)",
];

pub type EntityChunkRead = (ChunkPos, io::Result<Option<Nbt>>);

pub struct EntityStorage {
    worker: Option<IoWorker>,
    empty_chunks: FastSet<i64>,
    reads: (Sender<EntityChunkRead>, Receiver<EntityChunkRead>),
    loading_inbox: Arc<AtomicUsize>,
}

impl EntityStorage {
    pub fn new(storage: Option<RegionStorage>) -> EntityStorage {
        EntityStorage { worker: storage.map(IoWorker::new), empty_chunks: FastSet::default(), reads: channel(), loading_inbox: Arc::default() }
    }

    pub fn load_entities(&mut self, pos: ChunkPos) {
        let Some(worker) = self.worker.as_ref().filter(|_| !self.empty_chunks.contains(&pos.pack())) else {
            self.loading_inbox.fetch_add(1, Ordering::Relaxed);
            let _ = self.reads.0.send((pos, Ok(None)));
            return;
        };
        let reads = self.reads.0.clone();
        let loading_inbox = self.loading_inbox.clone();
        worker.read(pos, move |read| {
            loading_inbox.fetch_add(1, Ordering::Relaxed);
            drop(reads.send((pos, read)));
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

pub fn chunk_tag(pos: ChunkPos, entities: Vec<Nbt>) -> Nbt {
    Nbt::Compound(vec![
        ("DataVersion".into(), Nbt::Int(WORLD_VERSION)),
        ("Entities".into(), Nbt::List(entities)),
        ("Position".into(), Nbt::IntArray(vec![pos.x, pos.z])),
    ])
}

impl ServerLevel {
    pub fn read_chunk_entities(&mut self, pos: ChunkPos, tag: &Nbt) -> Vec<LevelEntity> {
        match tag.get("Position") {
            Some(Nbt::IntArray(stored)) if stored.len() == 2 => {
                if *stored != [pos.x, pos.z] {
                    let (x, z) = (pos.x, pos.z);
                    error!("Chunk file at [{x}, {z}] is in the wrong location. (Expected [{x}, {z}], got [{}, {}])", stored[0], stored[1]);
                }
            }
            _ => warn!("Failed to parse chunk [{}, {}] position info", pos.x, pos.z),
        }
        let upgraded_chunk_tag = super::simple_region_storage::upgrade_chunk_tag_to_current_version(tag, -1, crate::util::datafix::data_fix_types::DataFixTypes::EntityChunk);
        let tag = &*upgraded_chunk_tag;
        let _collector = tag_value_input::ScopedCollector::new(PathElement::Text(format!("chunk@[{}, {}]", pos.x, pos.z)), ENTITY_STORAGE_LOGGER);
        let mut site = LOAD_ENTITIES_RECURSIVE.to_vec();
        site.extend(main_thread_task_frames(self.preparing_levels));
        let mut loaded = Vec::new();
        tag_value_input::for_each_child(tag, "Entities", |entity| loaded.extend(self.load_entity_recursive_in_scope(entity, &site, &mut |_| {})));
        loaded
    }
}
