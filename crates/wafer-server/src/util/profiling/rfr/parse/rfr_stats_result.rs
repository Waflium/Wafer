use crate::util::profiling::rfr::serialize::rfr_result_json_serializer::RfrResultJsonSerializer;
use crate::util::profiling::rfr::stats::chunk_gen_stat::ChunkGenStat;
use crate::util::profiling::rfr::stats::chunk_identification::ChunkIdentification;
use crate::util::profiling::rfr::stats::cpu_load_stat::CpuLoadStat;
use crate::util::profiling::rfr::stats::file_io_stat;
use crate::util::profiling::rfr::stats::fps_stat::FpsStat;
use crate::util::profiling::rfr::stats::heap_stat;
use crate::util::profiling::rfr::stats::io_summary::IoSummary;
use crate::util::profiling::rfr::stats::packet_identification::PacketIdentification;
use crate::util::profiling::rfr::stats::structure_gen_stat::StructureGenStat;
use crate::util::profiling::rfr::stats::thread_allocation_stat;
use crate::util::profiling::rfr::stats::tick_time_stat::TickTimeStat;
use crate::util::profiling::rfr::stats::timed_stat_summary::TimedStatSummary;

pub struct RfrStatsResult {
    pub recording_started: i64,
    pub recording_ended: i64,
    pub recording_duration: i64,
    pub world_creation_duration: Option<i64>,
    pub fps: Vec<FpsStat>,
    pub server_tick_times: Vec<TickTimeStat>,
    pub cpu_load_stats: Vec<CpuLoadStat>,
    pub heap_summary: heap_stat::Summary,
    pub thread_allocation_summary: thread_allocation_stat::Summary,
    pub received_packets_summary: IoSummary<PacketIdentification>,
    pub sent_packets_summary: IoSummary<PacketIdentification>,
    pub written_chunks: IoSummary<ChunkIdentification>,
    pub read_chunks: IoSummary<ChunkIdentification>,
    pub file_writes: file_io_stat::Summary,
    pub file_reads: file_io_stat::Summary,
    pub chunk_gen_stats: Vec<ChunkGenStat>,
    pub structure_gen_stats: Vec<StructureGenStat>,
}

impl RfrStatsResult {
    pub fn chunk_gen_summary(&self) -> Vec<(&'static str, TimedStatSummary<ChunkGenStat>)> {
        let mut by_status: Vec<(&'static str, Vec<ChunkGenStat>)> = Vec::new();
        for stat in &self.chunk_gen_stats {
            match by_status.iter_mut().find(|(status, _)| *status == stat.status) {
                Some((_, stats)) => stats.push(stat.clone()),
                None => by_status.push((stat.status, vec![stat.clone()])),
            }
        }
        let mut summaries: Vec<(&'static str, TimedStatSummary<ChunkGenStat>)> = by_status.into_iter().filter_map(|(status, stats)| TimedStatSummary::summary(&stats).map(|summary| (status, summary))).collect();
        summaries.sort_by(|(_, a), (_, b)| b.total_duration.cmp(&a.total_duration));
        summaries
    }

    pub fn as_json(&self) -> Result<String, String> {
        RfrResultJsonSerializer.format(self)
    }
}
