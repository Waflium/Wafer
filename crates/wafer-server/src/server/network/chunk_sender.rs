use wafer_core::ChunkPos;
use wafer_core::fastutil::LongOpenHashSet;

use crate::util::top_k_selector;

const MIN_CHUNKS_PER_TICK: f32 = 0.01;
const MAX_CHUNKS_PER_TICK: f32 = 64.0;
const START_CHUNKS_PER_TICK: f32 = 9.0;
const MAX_UNACKNOWLEDGED_BATCHES: i32 = 10;

pub struct PlayerChunkSender {
    pending: LongOpenHashSet,
    desired_chunks_per_tick: f32,
    batch_quota: f32,
    unacknowledged_batches: i32,
    max_unacknowledged_batches: i32,
}

impl Default for PlayerChunkSender {
    fn default() -> Self {
        PlayerChunkSender {
            pending: LongOpenHashSet::new(),
            desired_chunks_per_tick: START_CHUNKS_PER_TICK,
            batch_quota: 0.0,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
        }
    }
}

impl PlayerChunkSender {
    pub fn mark_chunk_pending_to_send(&mut self, pos: ChunkPos) {
        self.pending.add(pos.pack());
    }

    pub fn drop_chunk(&mut self, pos: ChunkPos) -> bool {
        self.pending.remove(pos.pack())
    }

    pub fn is_pending(&self, pos: ChunkPos) -> bool {
        self.pending.contains(pos.pack())
    }

    pub fn next_batch(&mut self, player_chunk: ChunkPos, ready: impl Fn(ChunkPos) -> bool) -> Vec<ChunkPos> {
        if self.unacknowledged_batches >= self.max_unacknowledged_batches {
            return Vec::new();
        }
        let max_batch_size = self.desired_chunks_per_tick.max(1.0);
        self.batch_quota = (self.batch_quota + self.desired_chunks_per_tick).min(max_batch_size);
        if self.batch_quota < 1.0 || self.pending.is_empty() {
            return Vec::new();
        }
        let max_batch_size = self.batch_quota.floor() as usize;
        let distance = |c: &ChunkPos| (c.x - player_chunk.x).pow(2) + (c.z - player_chunk.z).pow(2);
        let pending = self.pending.stream().map(ChunkPos::unpack);
        let chunks: Vec<ChunkPos> = if self.pending.len() > max_batch_size {
            top_k_selector::least(max_batch_size, pending, |a, b| distance(a).cmp(&distance(b))).into_iter().filter(|&c| ready(c)).collect()
        } else {
            let mut chunks: Vec<ChunkPos> = pending.filter(|&c| ready(c)).collect();
            chunks.sort_by_key(distance);
            chunks
        };
        for c in &chunks {
            self.pending.remove(c.pack());
        }
        if chunks.is_empty() {
            return chunks;
        }
        self.unacknowledged_batches += 1;
        self.batch_quota -= chunks.len() as f32;
        chunks
    }

    pub fn on_chunk_batch_received(&mut self, desired_chunks_per_tick: f32) {
        self.unacknowledged_batches = (self.unacknowledged_batches - 1).max(0);
        self.desired_chunks_per_tick =
            if desired_chunks_per_tick.is_nan() { MIN_CHUNKS_PER_TICK } else { desired_chunks_per_tick.clamp(MIN_CHUNKS_PER_TICK, MAX_CHUNKS_PER_TICK) };
        if self.unacknowledged_batches == 0 {
            self.batch_quota = 1.0;
        }
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
    }
}
