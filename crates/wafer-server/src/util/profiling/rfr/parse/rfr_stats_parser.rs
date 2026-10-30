use std::path::Path;

use super::rfr_stats_result::RfrStatsResult;
use crate::rfr::consumer::recording_file::{RecordedEvent, RecordingFile};
use crate::util::java_collections::java_hash_map_iteration_order;
use crate::util::profiling::rfr::stats::chunk_gen_stat::ChunkGenStat;
use crate::util::profiling::rfr::stats::chunk_identification::ChunkIdentification;
use crate::util::profiling::rfr::stats::cpu_load_stat::CpuLoadStat;
use crate::util::profiling::rfr::stats::file_io_stat::FileIOStat;
use crate::util::profiling::rfr::stats::fps_stat::FpsStat;
use crate::util::profiling::rfr::stats::heap_stat::HeapStat;
use crate::util::profiling::rfr::stats::io_summary::{CountAndSize, IoSummary};
use crate::util::profiling::rfr::stats::packet_identification::PacketIdentification;
use crate::util::profiling::rfr::stats::structure_gen_stat::StructureGenStat;
use crate::util::profiling::rfr::stats::thread_allocation_stat::ThreadAllocationStat;
use crate::util::profiling::rfr::stats::tick_time_stat::TickTimeStat;

#[derive(Default)]
pub struct RfrStatsParser {
    recording_started: i64,
    recording_ended: i64,
    chunk_gen_stats: Vec<ChunkGenStat>,
    structure_gen_stats: Vec<StructureGenStat>,
    cpu_load_stat: Vec<CpuLoadStat>,
    received_packets: Vec<(PacketIdentification, CountAndSize)>,
    sent_packets: Vec<(PacketIdentification, CountAndSize)>,
    read_chunks: Vec<(ChunkIdentification, CountAndSize)>,
    written_chunks: Vec<(ChunkIdentification, CountAndSize)>,
    file_writes: Vec<FileIOStat>,
    file_reads: Vec<FileIOStat>,
    garbage_collections: i32,
    gc_total_duration: i64,
    heap_stats: Vec<HeapStat>,
    thread_allocation_stats: Vec<ThreadAllocationStat>,
    fps: Vec<FpsStat>,
    server_tick_times: Vec<TickTimeStat>,
    world_creation_duration: Option<i64>,
}

impl RfrStatsParser {
    pub fn parse(path: &Path) -> Result<RfrStatsResult, String> {
        let mut recording_file = RecordingFile::open(path).map_err(|e| e.to_string())?;
        let ticks_per_second = recording_file.ticks_per_second;
        let mut parser = RfrStatsParser::default();
        while recording_file.has_more_events() {
            let Some(event) = recording_file.read_event() else { break };
            parser.capture(&event, ticks_per_second);
        }
        parser.results()
    }

    fn results(self) -> Result<RfrStatsResult, String> {
        let recording_duration = self.recording_ended - self.recording_started;
        Ok(RfrStatsResult {
            recording_started: self.recording_started,
            recording_ended: self.recording_ended,
            recording_duration,
            world_creation_duration: self.world_creation_duration,
            fps: self.fps,
            server_tick_times: self.server_tick_times,
            cpu_load_stats: self.cpu_load_stat,
            heap_summary: HeapStat::summary(recording_duration, &self.heap_stats, self.gc_total_duration, self.garbage_collections)?,
            thread_allocation_summary: ThreadAllocationStat::summary(&self.thread_allocation_stats),
            received_packets_summary: collect_io_stats(recording_duration, self.received_packets, PacketIdentification::hash_code),
            sent_packets_summary: collect_io_stats(recording_duration, self.sent_packets, PacketIdentification::hash_code),
            written_chunks: collect_io_stats(recording_duration, self.written_chunks, ChunkIdentification::hash_code),
            read_chunks: collect_io_stats(recording_duration, self.read_chunks, ChunkIdentification::hash_code),
            file_writes: FileIOStat::summary(recording_duration, &self.file_writes),
            file_reads: FileIOStat::summary(recording_duration, &self.file_reads),
            chunk_gen_stats: self.chunk_gen_stats,
            structure_gen_stats: self.structure_gen_stats,
        })
    }

    fn capture(&mut self, event: &RecordedEvent, ticks_per_second: i64) {
        if event.get_end_time() > self.recording_ended || self.recording_ended == 0 {
            self.recording_ended = event.get_end_time();
        }
        if event.get_start_time() < self.recording_started || self.recording_started == 0 {
            self.recording_started = event.get_start_time();
        }
        match event.get_event_type_name() {
            "minecraft.ChunkGeneration" => self.chunk_gen_stats.push(ChunkGenStat::from(event)),
            "minecraft.StructureGeneration" => self.structure_gen_stats.push(StructureGenStat::from(event)),
            "minecraft.LoadWorld" => self.world_creation_duration = Some(event.get_duration().as_nanos() as i64),
            "minecraft.ClientFps" => self.fps.push(FpsStat::from(event, "fps")),
            "minecraft.ServerTickTime" => self.server_tick_times.push(TickTimeStat::from(event, ticks_per_second)),
            "minecraft.PacketReceived" => increment(&mut self.received_packets, PacketIdentification::from(event), event.get_int("bytes")),
            "minecraft.PacketSent" => increment(&mut self.sent_packets, PacketIdentification::from(event), event.get_int("bytes")),
            "minecraft.ChunkRegionRead" => increment(&mut self.read_chunks, ChunkIdentification::from(event), event.get_int("bytes")),
            "minecraft.ChunkRegionWrite" => increment(&mut self.written_chunks, ChunkIdentification::from(event), event.get_int("bytes")),
            "rust.ThreadAllocationStatistics" => self.thread_allocation_stats.push(ThreadAllocationStat::from(event)),
            "rust.HeapSummary" => self.heap_stats.push(HeapStat::from(event)),
            "rust.CPULoad" => self.cpu_load_stat.push(CpuLoadStat::from(event)),
            "rust.FileWrite" => self.file_writes.push(FileIOStat { duration: event.get_duration().as_nanos() as i64, path: event.get_string("path"), bytes: event.get_long("bytesWritten") }),
            "rust.FileRead" => self.file_reads.push(FileIOStat { duration: event.get_duration().as_nanos() as i64, path: event.get_string("path"), bytes: event.get_long("bytesRead") }),
            _ => {}
        }
    }
}

fn increment<T: PartialEq>(stats: &mut Vec<(T, CountAndSize)>, identification: T, bytes: i32) {
    let index = match stats.iter().position(|(existing, _)| *existing == identification) {
        Some(index) => index,
        None => {
            stats.push((identification, CountAndSize::default()));
            stats.len() - 1
        }
    };
    let stat = &mut stats[index].1;
    stat.total_size += bytes as i64;
    stat.total_count += 1;
}

fn collect_io_stats<T: Clone>(recording_duration: i64, stats: Vec<(T, CountAndSize)>, hash_code: fn(&T) -> i32) -> IoSummary<T> {
    let hashes: Vec<i32> = stats.iter().map(|(identification, _)| hash_code(identification)).collect();
    let ordered = java_hash_map_iteration_order(&hashes).into_iter().map(|i| stats[i].clone()).collect();
    IoSummary::new(recording_duration, ordered)
}
