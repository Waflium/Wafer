use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::util::java_throwable::JavaThrowable;

const LATEST_ENTRY_COUNT: usize = 8;

struct LongEntry {
    timestamp_ms: i64,
    location: String,
    cls: String,
    message: Option<String>,
}

struct ShortEntry {
    location: String,
    cls: String,
}

#[derive(Default)]
struct Entries {
    latest_entries: VecDeque<LongEntry>,
    entry_counts: Vec<(ShortEntry, i32)>,
}

#[derive(Default)]
pub struct SuppressedExceptionCollector {
    entries: Mutex<Entries>,
}

fn current_time_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl SuppressedExceptionCollector {
    pub fn add_entry(&self, location: &str, throwable: &JavaThrowable) {
        let now = current_time_ms();
        let message = throwable.message().map(str::to_string);
        let cls = format!("class {}", throwable.class_name());
        let mut entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        entries.latest_entries.push_back(LongEntry { timestamp_ms: now, location: location.to_string(), cls: cls.clone(), message });
        while entries.latest_entries.len() > LATEST_ENTRY_COUNT {
            entries.latest_entries.pop_front();
        }
        let current_value = match entries.entry_counts.iter().position(|(key, _)| key.location == location && key.cls == cls) {
            Some(index) => entries.entry_counts.remove(index).1,
            None => 0,
        };
        entries.entry_counts.insert(0, (ShortEntry { location: location.to_string(), cls }, current_value + 1));
    }

    pub fn dump(&self) -> String {
        let current = current_time_ms();
        let entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut result = String::new();
        if !entries.latest_entries.is_empty() {
            result.push_str("\n\t\tLatest entries:\n");
            for e in &entries.latest_entries {
                result.push_str(&format!("\t\t\t{}:{}: {} ({}ms ago)\n", e.location, e.cls, e.message.as_deref().unwrap_or("null"), current - e.timestamp_ms));
            }
        }
        if !entries.entry_counts.is_empty() {
            if result.is_empty() {
                result.push('\n');
            }
            result.push_str("\t\tEntry counts:\n");
            for (key, count) in &entries.entry_counts {
                result.push_str(&format!("\t\t\t{}:{} x {}\n", key.location, key.cls, count));
            }
        }
        if result.is_empty() { "~~NONE~~".to_string() } else { result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_nothing_without_entries() {
        assert_eq!(SuppressedExceptionCollector::default().dump(), "~~NONE~~");
    }

    #[test]
    fn dumps_latest_entries_and_counts_with_the_most_recent_count_first() {
        let collector = SuppressedExceptionCollector::default();
        let zip = JavaThrowable::new("java.util.zip.ZipException: incorrect header check", Vec::new());
        let eof = JavaThrowable::new("java.io.EOFException", Vec::new());
        collector.add_entry("chunk/load", &zip);
        collector.add_entry("chunk/save", &eof);
        collector.add_entry("chunk/load", &zip);
        let dump = collector.dump();
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "\t\tLatest entries:");
        assert!(lines[2].starts_with("\t\t\tchunk/load:class java.util.zip.ZipException: incorrect header check ("));
        assert!(lines[3].starts_with("\t\t\tchunk/save:class java.io.EOFException: null ("));
        assert!(lines[4].starts_with("\t\t\tchunk/load:class java.util.zip.ZipException: incorrect header check ("));
        assert_eq!(&lines[5..], ["\t\tEntry counts:", "\t\t\tchunk/load:class java.util.zip.ZipException x 2", "\t\t\tchunk/save:class java.io.EOFException x 1"]);
    }

    #[test]
    fn keeps_only_the_latest_eight_entries() {
        let collector = SuppressedExceptionCollector::default();
        for i in 0..10 {
            collector.add_entry(&format!("packet/{i}"), &JavaThrowable::new("java.lang.IllegalStateException: x", Vec::new()));
        }
        let dump = collector.dump();
        assert_eq!(dump.matches("ms ago)").count(), 8);
        let latest = dump.split("Entry counts:").next().unwrap();
        assert!(!latest.contains("packet/0:"));
        assert!(latest.contains("packet/2:"));
        assert!(dump.contains("packet/0:class java.lang.IllegalStateException x 1"));
    }
}
