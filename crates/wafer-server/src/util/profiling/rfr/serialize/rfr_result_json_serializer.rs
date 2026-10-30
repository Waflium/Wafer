use crate::nbt::{java_double_to_string, java_float_to_string};
use crate::util::gson::JsonElement;
use crate::util::gson::json_writer::{escape_html_safe, write_pretty};
use crate::util::java_collections::{java_hash_map_iteration_order, java_string_hash_code};
use crate::util::profiling::rfr::parse::rfr_stats_result::RfrStatsResult;
use crate::util::profiling::rfr::percentiles;
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
use crate::util::profiling::rfr::stats::millis;

pub struct RfrResultJsonSerializer;

fn double_sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut sum, mut compensation, mut simple_sum) = (0.0f64, 0.0f64, 0.0f64);
    for value in values {
        simple_sum += value;
        let tmp = value - compensation;
        let velvel = sum + tmp;
        compensation = (velvel - sum) - tmp;
        sum = velvel;
    }
    let tmp = sum - compensation;
    if tmp.is_nan() && simple_sum.is_infinite() { simple_sum } else { tmp }
}

struct JsonObject(Vec<(String, JsonElement)>);

impl JsonObject {
    fn new() -> JsonObject {
        JsonObject(Vec::new())
    }

    fn add(&mut self, name: &str, value: JsonElement) {
        match self.0.iter_mut().find(|(existing, _)| existing == name) {
            Some((_, existing)) => *existing = value,
            None => self.0.push((name.to_string(), value)),
        }
    }

    fn long(&mut self, name: &str, value: i64) {
        self.add(name, JsonElement::Number(value.to_string()));
    }

    fn double(&mut self, name: &str, value: f64) {
        self.add(name, JsonElement::Number(java_double_to_string(value)));
    }

    fn float(&mut self, name: &str, value: f32) {
        self.add(name, JsonElement::Number(java_float_to_string(value)));
    }

    fn string(&mut self, name: &str, value: Option<&str>) {
        self.add(name, value.map_or(JsonElement::Null, |value| JsonElement::String(value.to_string())));
    }

    fn boolean(&mut self, name: &str, value: bool) {
        self.add(name, JsonElement::Boolean(value));
    }

    fn build(self) -> JsonElement {
        JsonElement::Object(self.0)
    }
}

fn serialize_packet_id(identifier: &PacketIdentification, output: &mut JsonObject) {
    output.string("protocolId", identifier.protocol_id.as_deref());
    output.string("packetId", identifier.packet_id.as_deref());
}

fn serialize_chunk_id(identifier: &ChunkIdentification, output: &mut JsonObject) {
    output.string("level", identifier.level.as_deref());
    output.string("dimension", identifier.dimension.as_deref());
    output.long("x", identifier.x as i64);
    output.long("z", identifier.z as i64);
}

fn percentiles_object(percentiles: &[(i32, f64)]) -> JsonElement {
    let mut json = JsonObject::new();
    for (percentile, value) in percentiles {
        json.double(&format!("p{percentile}"), *value);
    }
    json.build()
}

impl RfrResultJsonSerializer {
    pub fn format(&self, stats: &RfrStatsResult) -> Result<String, String> {
        let mut root = JsonObject::new();
        root.long("startedEpoch", stats.recording_started.div_euclid(1_000_000));
        root.long("endedEpoch", stats.recording_ended.div_euclid(1_000_000));
        root.long("durationMs", millis(stats.recording_duration));
        if let Some(world_creation_duration) = stats.world_creation_duration {
            root.long("worldGenDurationMs", millis(world_creation_duration));
        }
        root.add("heap", self.heap(&stats.heap_summary));
        root.add("cpuPercent", self.cpu(&stats.cpu_load_stats));
        root.add("network", self.network(stats));
        root.add("fileIO", self.file_io(stats));
        root.add("fps", self.fps(&stats.fps));
        root.add("serverTick", self.server_ticks(&stats.server_tick_times));
        root.add("threadAllocation", self.thread_allocations(&stats.thread_allocation_summary));
        root.add("chunkGen", self.chunk_gen(&stats.chunk_gen_summary()));
        root.add("structureGen", self.structure_gen(&stats.structure_gen_stats)?);
        let mut out = String::new();
        write_pretty(&mut out, &root.build(), 0);
        Ok(escape_html_safe(&out))
    }

    fn heap(&self, heap_summary: &heap_stat::Summary) -> JsonElement {
        let mut json = JsonObject::new();
        json.double("allocationRateBytesPerSecond", heap_summary.allocation_rate_bytes_per_second);
        json.long("gcCount", heap_summary.total_gcs as i64);
        json.float("gcOverHeadPercent", heap_summary.gc_over_head());
        json.long("gcTotalDurationMs", millis(heap_summary.gc_total_duration));
        json.build()
    }

    fn structure_gen(&self, structure_gen_stats: &[StructureGenStat]) -> Result<JsonElement, String> {
        let mut root = JsonObject::new();
        let Some(summary) = TimedStatSummary::summary(structure_gen_stats) else { return Ok(root.build()) };
        let mut structure_json_array = Vec::new();
        let mut by_name: Vec<(String, Vec<StructureGenStat>)> = Vec::new();
        for stat in structure_gen_stats {
            let name = stat.structure_name.clone().ok_or_else(|| "element cannot be mapped to a null key".to_string())?;
            match by_name.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, stats)) => stats.push(stat.clone()),
                None => by_name.push((name, vec![stat.clone()])),
            }
        }
        let generator = |structure_gen: &StructureGenStat| {
            let mut result = JsonObject::new();
            result.long("durationNanos", structure_gen.duration);
            result.long("chunkPosX", structure_gen.chunk_pos.0 as i64);
            result.long("chunkPosZ", structure_gen.chunk_pos.1 as i64);
            result.string("structureName", structure_gen.structure_name.as_deref());
            result.string("level", structure_gen.level.as_deref());
            result.boolean("success", structure_gen.success);
            result.build()
        };
        let hashes: Vec<i32> = by_name.iter().map(|(name, _)| java_string_hash_code(Some(name))).collect();
        let mut any = false;
        for index in java_hash_map_iteration_order(&hashes) {
            let (structure_name, timed_stat) = &by_name[index];
            let Some(stat_summary) = TimedStatSummary::summary(timed_stat) else { continue };
            let mut structure_json = JsonObject::new();
            structure_json.string("name", Some(structure_name));
            structure_json.long("count", stat_summary.count as i64);
            structure_json.long("durationNanosTotal", stat_summary.total_duration);
            structure_json.long("durationNanosAvg", stat_summary.total_duration / stat_summary.count as i64);
            structure_json.add("durationNanosPercentiles", percentiles_object(&stat_summary.percentiles_nanos));
            structure_json_array.push(structure_json.build());
            any = true;
        }
        root.add("structure", JsonElement::Array(structure_json_array));
        if any {
            root.add("fastest", generator(&summary.fastest));
            root.add("slowest", generator(&summary.slowest));
            root.add("secondSlowest", summary.second_slowest.as_ref().map_or(JsonElement::Null, generator));
        }
        Ok(root.build())
    }

    fn chunk_gen(&self, chunk_gen_summary: &[(&'static str, TimedStatSummary<ChunkGenStat>)]) -> JsonElement {
        let mut json = JsonObject::new();
        if chunk_gen_summary.is_empty() {
            return json.build();
        }
        json.double("durationNanosTotal", double_sum(chunk_gen_summary.iter().map(|(_, summary)| summary.total_duration as f64)));
        let generator = |chunk: &ChunkGenStat| {
            let mut result = JsonObject::new();
            result.long("durationNanos", chunk.duration);
            result.string("level", chunk.level.as_deref());
            result.long("chunkPosX", chunk.chunk_pos.0 as i64);
            result.long("chunkPosZ", chunk.chunk_pos.1 as i64);
            result.long("worldPosX", chunk.world_pos.0 as i64);
            result.long("worldPosZ", chunk.world_pos.1 as i64);
            result.build()
        };
        let mut chunk_json_array = Vec::new();
        for (status, chunk_stat) in chunk_gen_summary {
            let mut chunk_status_json = JsonObject::new();
            chunk_status_json.string("state", Some(status));
            chunk_status_json.long("count", chunk_stat.count as i64);
            chunk_status_json.long("durationNanosTotal", chunk_stat.total_duration);
            chunk_status_json.long("durationNanosAvg", chunk_stat.total_duration / chunk_stat.count as i64);
            chunk_status_json.add("durationNanosPercentiles", percentiles_object(&chunk_stat.percentiles_nanos));
            chunk_status_json.add("fastest", generator(&chunk_stat.fastest));
            chunk_status_json.add("slowest", generator(&chunk_stat.slowest));
            chunk_status_json.add("secondSlowest", chunk_stat.second_slowest.as_ref().map_or(JsonElement::Null, generator));
            chunk_json_array.push(chunk_status_json.build());
        }
        json.add("status", JsonElement::Array(chunk_json_array));
        json.build()
    }

    fn thread_allocations(&self, thread_allocation_summary: &thread_allocation_stat::Summary) -> JsonElement {
        JsonElement::Array(
            thread_allocation_summary
                .allocations_per_second_by_thread
                .iter()
                .map(|(thread_name, bytes_per_second)| {
                    let mut json = JsonObject::new();
                    json.string("thread", Some(thread_name));
                    json.double("bytesPerSecond", *bytes_per_second);
                    json.build()
                })
                .collect(),
        )
    }

    fn server_ticks(&self, tick_time_stats: &[TickTimeStat]) -> JsonElement {
        if tick_time_stats.is_empty() {
            return JsonElement::Null;
        }
        let mut json = JsonObject::new();
        let tick_times_ms: Vec<f64> = tick_time_stats.iter().map(|stat| stat.current_average as f64 / 1000000.0).collect();
        json.double("minMs", tick_times_ms.iter().copied().fold(f64::INFINITY, f64::min));
        json.double("averageMs", double_sum(tick_times_ms.iter().copied()) / tick_times_ms.len() as f64);
        json.double("maxMs", tick_times_ms.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        for (percentile, value) in percentiles::evaluate(&tick_times_ms) {
            json.double(&format!("p{percentile}"), value);
        }
        json.build()
    }

    fn fps(&self, fps_stats: &[FpsStat]) -> JsonElement {
        if fps_stats.is_empty() {
            return JsonElement::Null;
        }
        let mut json = JsonObject::new();
        let fps: Vec<i32> = fps_stats.iter().map(|stat| stat.fps).collect();
        json.long("minFps", *fps.iter().min().unwrap() as i64);
        json.double("averageFps", fps.iter().map(|&value| value as i64).sum::<i64>() as f64 / fps.len() as f64);
        json.long("maxFps", *fps.iter().max().unwrap() as i64);
        for (percentile, value) in percentiles::evaluate_ints(&fps) {
            json.double(&format!("p{percentile}"), value);
        }
        json.build()
    }

    fn file_io(&self, stats: &RfrStatsResult) -> JsonElement {
        let mut json = JsonObject::new();
        json.add("write", self.file_io_summary(&stats.file_writes));
        json.add("read", self.file_io_summary(&stats.file_reads));
        json.add("chunksRead", self.io_summary(&stats.read_chunks, serialize_chunk_id));
        json.add("chunksWritten", self.io_summary(&stats.written_chunks, serialize_chunk_id));
        json.build()
    }

    fn file_io_summary(&self, io: &file_io_stat::Summary) -> JsonElement {
        let mut json = JsonObject::new();
        json.long("totalBytes", io.total_bytes);
        json.long("count", io.counts);
        json.double("bytesPerSecond", io.bytes_per_second);
        json.double("countPerSecond", io.counts_per_second);
        let top_contributors = io
            .top_ten_contributors_by_total_bytes
            .iter()
            .map(|(path, total_bytes)| {
                let mut contributor_json = JsonObject::new();
                contributor_json.string("path", Some(path));
                contributor_json.long("totalBytes", *total_bytes);
                contributor_json.build()
            })
            .collect();
        json.add("topContributors", JsonElement::Array(top_contributors));
        json.build()
    }

    fn network(&self, stats: &RfrStatsResult) -> JsonElement {
        let mut json = JsonObject::new();
        json.add("sent", self.io_summary(&stats.sent_packets_summary, serialize_packet_id));
        json.add("received", self.io_summary(&stats.received_packets_summary, serialize_packet_id));
        json.build()
    }

    fn io_summary<T: Clone>(&self, summary: &IoSummary<T>, element_writer: fn(&T, &mut JsonObject)) -> JsonElement {
        let mut json = JsonObject::new();
        json.long("totalBytes", summary.get_total_size());
        json.long("count", summary.get_total_count());
        json.double("bytesPerSecond", summary.get_size_per_second());
        json.double("countPerSecond", summary.get_counts_per_second());
        let top_contributors = summary
            .largest_size_contributors()
            .iter()
            .map(|(identifier, count_and_size)| {
                let mut contributor_json = JsonObject::new();
                element_writer(identifier, &mut contributor_json);
                contributor_json.long("totalBytes", count_and_size.total_size);
                contributor_json.long("count", count_and_size.total_count);
                contributor_json.float("averageSize", count_and_size.average_size());
                contributor_json.build()
            })
            .collect();
        json.add("topContributors", JsonElement::Array(top_contributors));
        json.build()
    }

    fn cpu(&self, cpu_stats: &[CpuLoadStat]) -> JsonElement {
        let mut json = JsonObject::new();
        let transformer = |extractor: fn(&CpuLoadStat) -> f64| {
            let mut json_group = JsonObject::new();
            let values: Vec<f64> = cpu_stats.iter().map(extractor).collect();
            json_group.double("min", values.iter().copied().fold(f64::INFINITY, f64::min));
            json_group.double("average", if values.is_empty() { 0.0 } else { double_sum(values.iter().copied()) / values.len() as f64 });
            json_group.double("max", values.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            json_group.build()
        };
        json.add("rust", transformer(|stat| stat.rust));
        json.add("userRust", transformer(|stat| stat.user_rust));
        json.add("system", transformer(|stat| stat.system));
        json.build()
    }
}
