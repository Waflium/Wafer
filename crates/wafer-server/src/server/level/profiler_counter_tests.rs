use std::sync::{Arc, Mutex};
use std::time::Duration;

use wafer_core::{AABB, BlockPos, ChunkPos};
use wafer_data::blocks;

use crate::server::level::chunk_cache::ChunkCache;
use crate::server::level::chunk_workers::IoWorker;
use crate::server::level::{ServerLevel, testing};
use crate::util::profiling::active_profiler::{ActiveProfiler, lock};
use crate::util::profiling::profiler;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::level::BlockGetter;
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::update_flags::UPDATE_CLIENTS;

fn level() -> ServerLevel {
    let mut level = testing::level();
    for x in -2..=2 {
        for z in -2..=2 {
            level.chunks.chunk(ChunkPos::new(x, z));
        }
    }
    level
}

fn counters(action: impl FnOnce()) -> Vec<(String, i64)> {
    let profiler = Arc::new(Mutex::new(ActiveProfiler::new(Box::new(|| 0), true)));
    {
        let _scope = profiler::use_fillers(vec![profiler.clone()]);
        action();
    }
    let profiler = lock(&profiler);
    let mut counters: Vec<(String, i64)> = profiler.get_entry("root").map(|entry| entry.counters.iter().map(|(name, value)| (name.to_string(), *value)).collect()).unwrap_or_default();
    counters.sort();
    counters
}

fn count(counters: &[(String, i64)], name: &str) -> i64 {
    counters.iter().find(|(counter, _)| counter == name).map_or(0, |(_, value)| *value)
}

fn pos_in(chunk: ChunkPos) -> BlockPos {
    BlockPos::new(chunk.x * 16 + 3, -61, chunk.z * 16 + 3)
}

#[test]
fn reading_a_block_counts_one_chunk_lookup_and_one_miss_per_new_chunk() {
    let mut level = level();
    level.block_state(pos_in(ChunkPos::new(0, 0)));
    let counted = counters(|| {
        level.block_state(pos_in(ChunkPos::new(0, 0)));
        level.block_state(pos_in(ChunkPos::new(0, 0)));
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (2, 0));
    let counted = counters(|| {
        level.block_state(pos_in(ChunkPos::new(-2, -2)));
        level.block_state(pos_in(ChunkPos::new(-2, -2)));
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (2, 1));
}

#[test]
fn a_precipitation_tick_counts_the_heightmap_chunk_and_the_biome_chunk() {
    let mut level = level();
    level.block_state(pos_in(ChunkPos::new(0, 0)));
    let counted = counters(|| {
        level.tick_precipitation(BlockPos::new(3, 0, 3));
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (2, 1));
}

#[test]
fn the_chunk_cache_keeps_one_entry_per_chunk_and_status() {
    let level = level();
    let pos = pos_in(ChunkPos::new(0, 0));
    level.get_biome(pos);
    let counted = counters(|| {
        level.get_biome(pos);
        level.get_biome(pos);
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (2, 0));
    let counted = counters(|| {
        level.chunks.chunk_without_loading(pos.chunk());
        level.chunks.chunk_without_loading(pos.chunk());
        level.get_biome(pos);
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (3, 1));
}

#[test]
fn difficulty_height_and_block_entity_changes_each_count_one_chunk_lookup() {
    use crate::world::level::chunk::HeightmapType;
    use crate::world::level::server_level_accessor::ServerLevelAccessor;
    let mut level = level();
    let pos = pos_in(ChunkPos::new(0, 0));
    let counted = counters(|| {
        level.current_difficulty_at(pos);
    });
    assert_eq!(count(&counted, "getChunk"), 1);
    let counted = counters(|| {
        level.get_height(HeightmapType::WorldSurface, pos.x, pos.z);
    });
    assert_eq!(count(&counted, "getChunk"), 1);
    let counted = counters(|| {
        level.block_entity_changed(pos);
    });
    assert_eq!(count(&counted, "getChunk"), 1);
}

#[test]
fn a_path_navigation_snapshot_counts_one_get_chunk_now_per_chunk_and_reads_for_free() {
    use crate::world::level::path_navigation_region::PathNavigationRegion;
    let level = level();
    let mut region = None;
    let counted = counters(|| {
        region = Some(PathNavigationRegion::snapshot(&level, BlockPos::new(-20, -61, -20), BlockPos::new(20, -50, 20)));
    });
    assert_eq!((count(&counted, "getChunkNow"), count(&counted, "getChunk")), (16, 0));
    let region = region.unwrap();
    let counted = counters(|| {
        region.get_block_state(BlockPos::new(3, -61, 3));
    });
    assert_eq!(count(&counted, "getChunk"), 0);
    let live = PathNavigationRegion::new(&level);
    let counted = counters(|| {
        live.get_block_state(BlockPos::new(3, -61, 3));
    });
    assert_eq!(count(&counted, "getChunk"), 1);
}

#[test]
fn reading_above_the_build_height_does_not_look_up_a_chunk() {
    let mut level = level();
    let counted = counters(|| {
        level.block_state(BlockPos::new(3, 400, 3));
    });
    assert_eq!(count(&counted, "getChunk"), 0);
}

#[test]
fn setting_a_block_counts_the_chunk_lookup_and_the_state_read_back() {
    let mut level = level();
    level.block_state(pos_in(ChunkPos::new(0, 0)));
    let counted = counters(|| {
        level.set_block(pos_in(ChunkPos::new(0, 0)), blocks::STONE.default_state(), UPDATE_CLIENTS | 16);
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (2, 0));
}

#[test]
fn get_chunk_now_counts_itself_and_feeds_the_chunk_cache() {
    let level = level();
    let chunk = ChunkPos::new(-2, -2);
    let counted = counters(|| {
        assert!(level.chunks.get_chunk_now(chunk).is_some());
        assert!(level.chunks.get_chunk_now(chunk).is_some());
    });
    assert_eq!((count(&counted, "getChunkNow"), count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (2, 0, 0));
    let mut level = level;
    let counted = counters(|| {
        level.block_state(pos_in(chunk));
    });
    assert_eq!((count(&counted, "getChunk"), count(&counted, "getChunkCacheMiss")), (1, 0));
}

#[test]
fn get_chunk_now_of_an_unloaded_chunk_counts_every_time_and_is_not_cached() {
    let level = level();
    let counted = counters(|| {
        assert!(level.chunks.get_chunk_now(ChunkPos::new(40, 40)).is_none());
        assert!(level.chunks.get_chunk_now(ChunkPos::new(40, 40)).is_none());
    });
    assert_eq!(count(&counted, "getChunkNow"), 2);
}

#[test]
fn collisions_count_the_chunk_lookup_without_loading() {
    let level = level();
    let area = AABB::new(0.2, -61.0, 0.2, 0.8, -60.0, 0.8);
    let counted = counters(|| {
        level.block_collisions(&area, &crate::world::phys::shapes::collision_context::CollisionContext::EMPTY, false);
    });
    assert!(count(&counted, "getChunk") >= 1);
    assert!(count(&counted, "getChunkCacheMiss") >= 1);
    assert!(count(&counted, "getChunkCacheMiss") <= count(&counted, "getChunk"));
    assert_eq!(count(&counted, "getChunkNow"), 0);
}

#[test]
fn every_entity_query_counts_a_get_entities_call() {
    let mut level = level();
    level.spawn_entity_type("minecraft:pig", BlockPos::new(1, -60, 1), EntitySpawnReason::Command).unwrap();
    let area = AABB::new(-8.0, -64.0, -8.0, 8.0, 0.0, 8.0);
    let counted = counters(|| {
        level.get_entities(None, &area);
        level.get_entities_matching(None, &area, |_, _| true);
        level.get_entities_of_type(|_, _| true, &area, |_, _| true);
        level.items_in(&area, None);
        level.get_living_entities(&area, |_, _| true);
        level.experience_orbs_in(&area, None);
    });
    assert_eq!(count(&counted, "getEntities"), 6);
    assert_eq!(count(&counted, "hasEntities"), 0);
}

#[test]
fn has_entities_counts_only_itself() {
    let mut level = level();
    level.spawn_entity_type("minecraft:pig", BlockPos::new(1, -60, 1), EntitySpawnReason::Command).unwrap();
    let area = AABB::new(-8.0, -64.0, -8.0, 8.0, 0.0, 8.0);
    let counted = counters(|| {
        level.has_entities(|_, _| true, &area, |_, _| true);
        assert!(!level.has_entities(|_, _| true, &AABB::new(100.0, -64.0, 100.0, 101.0, 0.0, 101.0), |_, _| true));
    });
    assert_eq!((count(&counted, "hasEntities"), count(&counted, "getEntities")), (2, 0));
}

#[test]
fn every_chunk_main_thread_task_counts_one_run_task() {
    let mut level = level();
    level.chunks.add_ticket_with_radius(crate::server::level::ticket::TicketType::Forced, ChunkPos::new(6, 6), 1);
    let mut ran = 0;
    let counted = counters(|| {
        for _ in 0..400 {
            if level.chunks.poll_task() {
                ran += 1;
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    });
    assert!(ran > 0);
    assert_eq!(count(&counted, "runTask"), ran);
}

#[test]
fn io_workers_report_their_queued_requests() {
    let dir = std::env::temp_dir().join(format!("wafer-io-queue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let worker = IoWorker::new(RegionStorage::new(dir.clone(), false));
    assert_eq!(worker.queue_size(), 0);
    worker.synchronize(false).unwrap();
    assert_eq!(worker.queue_size(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn chunk_caches_without_storage_report_empty_queues() {
    let cache: ChunkCache = testing::level().chunks;
    assert_eq!((cache.chunk_io_queue_size(), cache.poi_io_queue_size(), cache.worldgen_queue_size(), cache.light_queue_size()), (0, 0, 0, 0));
}
