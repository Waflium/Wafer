use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use wafer_core::ChunkPos;

use super::callback::profiled_duration::ProfiledDuration;
use super::environment::Environment;
use super::event::network_summary_event::{self, SumAggregation};
use super::event::{chunk_generation_event, chunk_region_read_event, chunk_region_write_event, client_fps_event, packet_received_event, packet_sent_event, server_tick_time_event, structure_generation_event, world_load_finished_event};
use super::summary_reporter::SummaryReporter;
use crate::rfr::{EventType, Recording, Value, recording};
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;

pub struct RfrProfiler {
    recording: Mutex<Option<Arc<Recording>>>,
    current_fps: AtomicI32,
    current_average_tick_time_server: AtomicU32,
    network_traffic_by_address: Mutex<Vec<(String, Arc<SumAggregation>)>>,
}

static CUSTOM_EVENTS: [&EventType; 10] = [
    &chunk_generation_event::TYPE,
    &chunk_region_read_event::TYPE,
    &chunk_region_write_event::TYPE,
    &packet_received_event::TYPE,
    &packet_sent_event::TYPE,
    &network_summary_event::TYPE,
    &server_tick_time_event::TYPE,
    &client_fps_event::TYPE,
    &structure_generation_event::TYPE,
    &world_load_finished_event::TYPE,
];

static INSTANCE: OnceLock<RfrProfiler> = OnceLock::new();

impl RfrProfiler {
    pub fn get_instance() -> &'static RfrProfiler {
        INSTANCE.get_or_init(|| {
            for event_type in CUSTOM_EVENTS {
                recording::register(event_type);
            }
            crate::rfr::runtime::register();
            register_periodic_events();
            RfrProfiler { recording: Mutex::new(None), current_fps: AtomicI32::new(0), current_average_tick_time_server: AtomicU32::new(0), network_traffic_by_address: Mutex::new(Vec::new()) }
        })
    }

    pub fn start(&'static self, environment: Environment) -> bool {
        if self.is_running() {
            warn!("Profiling already in progress");
            return false;
        }
        let start_timestamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string();
        let recording = Recording::new();
        recording.set_name(format!("{}-{}-{}", environment.get_description(), wafer_data::VERSION_NAME, start_timestamp));
        let destination = Path::new("debug").join(format!("{}-{}.rfr", environment.get_description(), start_timestamp));
        if let Err(exception) = crate::util::file_util::create_directories_safe(destination.parent().unwrap()) {
            warn!("Failed to start rfr profiling\njava.io.IOException: {exception}");
            return false;
        }
        recording.set_destination(destination.clone());
        recording.start();
        *self.recording.lock().unwrap() = Some(recording.clone());
        info!("Started flight recorder profiling id({}):name({}) - will dump to {} on exit or stop command", recording.get_id(), recording.get_name(), destination.display());
        true
    }

    pub fn stop(&'static self) -> Result<PathBuf, String> {
        let Some(recording) = self.recording.lock().unwrap().clone() else {
            return Err("Not currently profiling".to_string());
        };
        self.network_traffic_by_address.lock().unwrap().clear();
        let report = recording.get_destination().unwrap_or_default();
        if let Err(error) = recording.stop() {
            return Err(error.to_string());
        }
        let summary_reporter = SummaryReporter::new(|| *RfrProfiler::get_instance().recording.lock().unwrap() = None);
        summary_reporter.recording_stopped(recording.get_destination().as_deref());
        Ok(report)
    }

    pub fn is_running(&self) -> bool {
        self.recording.lock().unwrap().is_some()
    }

    pub fn on_server_tick(&self, current_average_tick_time: f32) {
        if server_tick_time_event::TYPE.is_enabled() {
            self.current_average_tick_time_server.store(current_average_tick_time.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn tracks_packets(&self) -> bool {
        packet_received_event::TYPE.is_enabled() || packet_sent_event::TYPE.is_enabled() || network_summary_event::TYPE.is_enabled()
    }

    pub fn on_packet_received(&self, protocol: &str, flow: &str, packet_id: &str, remote_address: &str, readable_bytes: i32) {
        if packet_received_event::TYPE.is_enabled() {
            packet_received_event::TYPE.commit_now(&[Value::String(protocol), Value::String(flow), Value::String(packet_id), Value::String(remote_address), Value::Int(readable_bytes)]);
        }
        if network_summary_event::TYPE.is_enabled() {
            self.network_stat_for(remote_address).track_received_packet(readable_bytes);
        }
    }

    pub fn on_packet_sent(&self, protocol: &str, flow: &str, packet_id: &str, remote_address: &str, written_bytes: i32) {
        if packet_sent_event::TYPE.is_enabled() {
            packet_sent_event::TYPE.commit_now(&[Value::String(protocol), Value::String(flow), Value::String(packet_id), Value::String(remote_address), Value::Int(written_bytes)]);
        }
        if network_summary_event::TYPE.is_enabled() {
            self.network_stat_for(remote_address).track_sent_packet(written_bytes);
        }
    }

    fn network_stat_for(&self, remote_address: &str) -> Arc<SumAggregation> {
        let mut traffic = self.network_traffic_by_address.lock().unwrap();
        if let Some((_, aggregation)) = traffic.iter().find(|(address, _)| address == remote_address) {
            return aggregation.clone();
        }
        let aggregation = Arc::new(SumAggregation::new(remote_address.to_string()));
        traffic.push((remote_address.to_string(), aggregation.clone()));
        aggregation
    }

    pub fn on_region_file_read(&self, info: &RegionStorageInfo, pos: ChunkPos, version: i32, read_bytes: i32) {
        if chunk_region_read_event::TYPE.is_enabled() {
            commit_chunk_region_io(&chunk_region_read_event::TYPE, info, pos, version, read_bytes);
        }
    }

    pub fn on_region_file_write(&self, info: &RegionStorageInfo, pos: ChunkPos, version: i32, written_bytes: i32) {
        if chunk_region_write_event::TYPE.is_enabled() {
            commit_chunk_region_io(&chunk_region_write_event::TYPE, info, pos, version, written_bytes);
        }
    }

    pub fn on_world_loaded_started(&self) -> Option<ProfiledDuration> {
        if !world_load_finished_event::TYPE.is_enabled() {
            return None;
        }
        let begin = crate::rfr::ticks();
        Some(Box::new(move |_| world_load_finished_event::TYPE.commit(begin, crate::rfr::ticks(), &[])))
    }

    pub fn on_chunk_generate(&self, pos: ChunkPos, dimension: &str, name: &str) -> Option<ProfiledDuration> {
        if !chunk_generation_event::TYPE.is_enabled() {
            return None;
        }
        let (dimension, name) = (dimension.to_string(), name.to_string());
        let begin = crate::rfr::ticks();
        Some(Box::new(move |_| {
            chunk_generation_event::TYPE.commit(
                begin,
                crate::rfr::ticks(),
                &[Value::Int(pos.min_block_x()), Value::Int(pos.min_block_z()), Value::Int(pos.x), Value::Int(pos.z), Value::String(&name), Value::String(&dimension)],
            )
        }))
    }

    pub fn on_structure_generate(&self, source_chunk_pos: ChunkPos, dimension: &str, structure: &str) -> Option<ProfiledDuration> {
        if !structure_generation_event::TYPE.is_enabled() {
            return None;
        }
        let (dimension, structure) = (dimension.to_string(), structure.to_string());
        let begin = crate::rfr::ticks();
        Some(Box::new(move |success| {
            structure_generation_event::TYPE.commit(
                begin,
                crate::rfr::ticks(),
                &[Value::Int(source_chunk_pos.x), Value::Int(source_chunk_pos.z), Value::String(&structure), Value::String(&dimension), Value::Boolean(success)],
            )
        }))
    }
}

fn commit_chunk_region_io(event_type: &EventType, info: &RegionStorageInfo, pos: ChunkPos, version: i32, bytes: i32) {
    let compression = format!("standard:{version}");
    let now = crate::rfr::ticks();
    event_type.commit(
        now,
        now,
        &[
            Value::Int(pos.x >> 5),
            Value::Int(pos.z >> 5),
            Value::Int(pos.x & 31),
            Value::Int(pos.z & 31),
            Value::Int(pos.x),
            Value::Int(pos.z),
            Value::String(&info.level),
            Value::String(&info.dimension),
            Value::String(&info.type_),
            Value::String(&compression),
            Value::Int(bytes),
        ],
    );
}

fn register_periodic_events() {
    recording::add_periodic_event(&client_fps_event::TYPE, Duration::from_secs(1), || {
        client_fps_event::TYPE.commit_now(&[Value::Int(RfrProfiler::get_instance().current_fps.load(Ordering::Relaxed))]);
    });
    recording::add_periodic_event(&server_tick_time_event::TYPE, Duration::from_secs(1), || {
        let average = f32::from_bits(RfrProfiler::get_instance().current_average_tick_time_server.load(Ordering::Relaxed));
        server_tick_time_event::TYPE.commit_now(&[Value::Long((1000000.0f32 * average) as i64)]);
    });
    recording::add_periodic_event(&network_summary_event::TYPE, Duration::from_secs(10), || {
        let traffic = std::mem::take(&mut *RfrProfiler::get_instance().network_traffic_by_address.lock().unwrap());
        for (_, aggregation) in traffic {
            aggregation.commit_event();
        }
    });
}

pub fn dump_on_exit() {
    if let Some(profiler) = INSTANCE.get()
        && profiler.is_running()
    {
        let _ = profiler.stop();
    }
}
