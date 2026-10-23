use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::server::notifications::NotificationManager;
use crate::util::datafix::data_fix_types::DataFixTypes;

#[derive(Default)]
pub struct FileFixStats {
    finished_operations: AtomicI32,
    total_operations: AtomicI32,
}

impl FileFixStats {
    pub fn finished_operations(&self) -> i32 {
        self.finished_operations.load(Ordering::SeqCst)
    }

    pub fn total_operations(&self) -> i32 {
        self.total_operations.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.finished_operations.store(0, Ordering::SeqCst);
        self.total_operations.store(0, Ordering::SeqCst);
    }

    pub fn get_progress(&self) -> f32 {
        if self.total_operations() == 0 { 0.0 } else { (self.finished_operations() as f32 / self.total_operations() as f32).clamp(0.0, 1.0) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Counting,
    Failed,
    Finished,
    Upgrading,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Type {
    Files,
    LegacyStructures,
    Regions,
}

impl Type {
    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Type::Files => "files",
            Type::LegacyStructures => "legacy_structures",
            Type::Regions => "regions",
        }
    }
}

pub struct UpgradeProgress {
    finished: AtomicBool,
    total_file_fix_stats: FileFixStats,
    type_file_fix_stats: FileFixStats,
    running_file_fixer_stats: FileFixStats,
    total_progress: AtomicU32,
    total_chunks: AtomicI32,
    converted: AtomicI32,
    skipped: AtomicI32,
    progress_map: Mutex<HashMap<String, f32>>,
    canceled: AtomicBool,
    data_fix_type: Mutex<Option<DataFixTypes>>,
    status: Mutex<Status>,
    type_: Mutex<Option<Type>>,
    last_logged_progress_time: AtomicI64,
    notification_service: NotificationManager,
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64)
}

impl UpgradeProgress {
    pub fn new(notification_service: NotificationManager) -> UpgradeProgress {
        UpgradeProgress {
            finished: AtomicBool::new(false),
            total_file_fix_stats: FileFixStats::default(),
            type_file_fix_stats: FileFixStats::default(),
            running_file_fixer_stats: FileFixStats::default(),
            total_progress: AtomicU32::new(0.0f32.to_bits()),
            total_chunks: AtomicI32::new(0),
            converted: AtomicI32::new(0),
            skipped: AtomicI32::new(0),
            progress_map: Mutex::new(HashMap::new()),
            canceled: AtomicBool::new(false),
            data_fix_type: Mutex::new(None),
            status: Mutex::new(Status::Counting),
            type_: Mutex::new(None),
            last_logged_progress_time: AtomicI64::new(0),
            notification_service,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    pub fn set_finished(&self, finished: bool) {
        self.finished.store(finished, Ordering::SeqCst);
    }

    pub fn get_total_file_fix_stats(&self) -> &FileFixStats {
        &self.total_file_fix_stats
    }

    pub fn get_type_file_fix_stats(&self) -> &FileFixStats {
        &self.type_file_fix_stats
    }

    pub fn get_running_file_fixer_stats(&self) -> &FileFixStats {
        &self.running_file_fixer_stats
    }

    pub fn add_total_file_fix_operations(&self, additional_file_fix_operations: i32) {
        self.total_file_fix_stats.total_operations.fetch_add(additional_file_fix_operations, Ordering::SeqCst);
        self.type_file_fix_stats.total_operations.fetch_add(additional_file_fix_operations, Ordering::SeqCst);
    }

    pub fn get_total_progress(&self) -> f32 {
        f32::from_bits(self.total_progress.load(Ordering::SeqCst))
    }

    pub fn set_total_progress(&self, total_progress: f32) {
        self.total_progress.store(total_progress.to_bits(), Ordering::SeqCst);
    }

    pub fn get_total_chunks(&self) -> i32 {
        self.total_chunks.load(Ordering::SeqCst)
    }

    pub fn add_total_chunks(&self, additional_total_chunks: i32) {
        self.total_chunks.fetch_add(additional_total_chunks, Ordering::SeqCst);
    }

    pub fn get_converted(&self) -> i32 {
        self.converted.load(Ordering::SeqCst)
    }

    pub fn set_dimension_progress(&self, dimension_key: &str, current_progress: f32) {
        self.progress_map.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(dimension_key.to_string(), current_progress);
    }

    pub fn get_dimension_progress(&self, dimension_key: &str) -> f32 {
        self.progress_map.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(dimension_key).copied().unwrap_or(0.0)
    }

    pub fn increment_converted(&self) {
        self.converted.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_skipped(&self) -> i32 {
        self.skipped.load(Ordering::SeqCst)
    }

    pub fn increment_skipped(&self) {
        self.skipped.fetch_add(1, Ordering::SeqCst);
    }

    pub fn increment_finished_operations(&self) {
        self.increment_finished_operations_by(1);
    }

    pub fn increment_finished_operations_by(&self, count: i32) {
        self.total_file_fix_stats.finished_operations.fetch_add(count, Ordering::SeqCst);
        self.type_file_fix_stats.finished_operations.fetch_add(count, Ordering::SeqCst);
        self.log_progress();
    }

    pub fn set_canceled(&self) {
        self.canceled.store(true, Ordering::SeqCst);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::SeqCst)
    }

    pub fn get_status(&self) -> Status {
        *self.status.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_status(&self, status: Status) {
        *self.status.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = status;
    }

    pub fn get_data_fix_type(&self) -> Option<DataFixTypes> {
        *self.data_fix_type.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_type(&self, type_: Type) {
        *self.type_.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(type_);
        self.type_file_fix_stats.reset();
    }

    pub fn get_type(&self) -> Option<Type> {
        *self.type_.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_applicable_fixer_amount(&self, amount: i32) {
        self.running_file_fixer_stats.total_operations.store(amount, Ordering::SeqCst);
    }

    pub fn increment_running_file_fixer(&self) {
        self.running_file_fixer_stats.finished_operations.fetch_add(1, Ordering::SeqCst);
    }

    pub fn reset(&self, data_fix_type: DataFixTypes) {
        self.total_file_fix_stats.reset();
        self.type_file_fix_stats.reset();
        self.total_chunks.store(0, Ordering::SeqCst);
        self.converted.store(0, Ordering::SeqCst);
        self.skipped.store(0, Ordering::SeqCst);
        *self.data_fix_type.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(data_fix_type);
    }

    pub fn log_progress(&self) {
        let now = now_millis();
        if now >= self.last_logged_progress_time.load(Ordering::SeqCst) + 1000 {
            let progress = self.total_file_fix_stats.finished_operations() as f32 / self.total_file_fix_stats.total_operations() as f32;
            self.last_logged_progress_time.store(now, Ordering::SeqCst);
            info!("Upgrading progress: {}%", (progress * 100.0f32) as i32);
            self.notification_service.world_upgrade_progress(progress);
        }
    }

    pub fn notifications(&self) -> &NotificationManager {
        &self.notification_service
    }
}
