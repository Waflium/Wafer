use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use super::active_profiler::PathEntry;
use crate::crash_report::ReportType;
use crate::util::java_collections::JavaHashMap;
use crate::util::java_format_fixed;

const PATH_SEPARATOR: char = '\u{1e}';

pub fn demangle_path(path: &str) -> String {
    path.replace(PATH_SEPARATOR, ".")
}

#[derive(Clone)]
pub enum ProfileResults {
    Empty,
    Filled(FilledProfileResults),
}

impl ProfileResults {
    pub fn is_empty(&self) -> bool {
        matches!(self, ProfileResults::Empty)
    }

    pub fn get_start_time_nano(&self) -> i64 {
        match self {
            ProfileResults::Empty => 0,
            ProfileResults::Filled(results) => results.start_time_nano,
        }
    }

    pub fn get_end_time_nano(&self) -> i64 {
        match self {
            ProfileResults::Empty => 0,
            ProfileResults::Filled(results) => results.end_time_nano,
        }
    }

    pub fn get_start_time_ticks(&self) -> i32 {
        match self {
            ProfileResults::Empty => 0,
            ProfileResults::Filled(results) => results.start_time_ticks,
        }
    }

    pub fn get_end_time_ticks(&self) -> i32 {
        match self {
            ProfileResults::Empty => 0,
            ProfileResults::Filled(results) => results.end_time_ticks,
        }
    }

    pub fn get_nano_duration(&self) -> i64 {
        self.get_end_time_nano() - self.get_start_time_nano()
    }

    pub fn get_tick_duration(&self) -> i32 {
        self.get_end_time_ticks() - self.get_start_time_ticks()
    }

    pub fn save_results(&self, file: &Path) -> bool {
        match self {
            ProfileResults::Empty => false,
            ProfileResults::Filled(results) => results.save_results(file),
        }
    }
}

pub struct ResultField {
    pub name: String,
    pub percentage: f64,
    pub global_percentage: f64,
    pub count: i64,
}

fn compare_java_strings(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

impl ResultField {
    fn compare_to(&self, other: &ResultField) -> Ordering {
        if other.percentage < self.percentage {
            Ordering::Less
        } else if other.percentage > self.percentage {
            Ordering::Greater
        } else {
            compare_java_strings(&other.name, &self.name)
        }
    }
}

#[derive(Clone)]
pub struct FilledProfileResults {
    entries: HashMap<String, PathEntry>,
    pub start_time_nano: i64,
    pub start_time_ticks: i32,
    pub end_time_nano: i64,
    pub end_time_ticks: i32,
    tick_duration: i32,
}

struct CounterCollector {
    self_value: i64,
    total_value: i64,
    children: JavaHashMap<CounterCollector>,
}

impl Default for CounterCollector {
    fn default() -> CounterCollector {
        CounterCollector { self_value: 0, total_value: 0, children: JavaHashMap::new(wafer_core::random::java_string_hash) }
    }
}

impl CounterCollector {
    fn add_value(&mut self, path: &mut std::slice::Iter<'_, &str>, value: i64) {
        self.total_value += value;
        match path.next() {
            None => self.self_value += value,
            Some(&segment) => {
                if !self.children.contains_key(segment) {
                    self.children.insert(segment.to_string(), CounterCollector::default());
                }
                self.children.get_mut(segment).expect("child").add_value(path, value);
            }
        }
    }
}

fn is_direct_child(path: &str, test: &str) -> bool {
    test.len() > path.len() && test.starts_with(path) && !test.as_bytes()[path.len() + 1..].contains(&(PATH_SEPARATOR as u8))
}

fn indent_line(builder: &mut String, depth: i32) {
    builder.push_str(&format!("[{depth:02}] "));
    for _ in 0..depth {
        builder.push_str("|   ");
    }
}

impl FilledProfileResults {
    pub fn new(entries: HashMap<String, PathEntry>, start_time_nano: i64, start_time_ticks: i32, end_time_nano: i64, end_time_ticks: i32) -> FilledProfileResults {
        FilledProfileResults { entries, start_time_nano, start_time_ticks, end_time_nano, end_time_ticks, tick_duration: end_time_ticks - start_time_ticks }
    }

    fn duration_of(&self, path: &str) -> i64 {
        self.entries.get(path).map_or(0, |entry| entry.accumulated_duration)
    }

    fn count_of(&self, path: &str) -> i64 {
        self.entries.get(path).map_or(0, |entry| entry.count)
    }

    pub fn get_times(&self, raw_path: &str) -> Vec<ResultField> {
        let mut global_time = self.duration_of("root");
        let self_time = self.duration_of(raw_path);
        let self_count = self.count_of(raw_path);
        let mut result: Vec<ResultField> = Vec::new();
        let mut path = raw_path.to_string();
        if !path.is_empty() {
            path.push(PATH_SEPARATOR);
        }
        let mut total_time: i64 = 0;
        for key in self.entries.keys() {
            if is_direct_child(&path, key) {
                total_time += self.duration_of(key);
            }
        }
        let old_time = total_time as f32;
        if total_time < self_time {
            total_time = self_time;
        }
        if global_time < total_time {
            global_time = total_time;
        }
        for key in self.entries.keys() {
            if is_direct_child(&path, key) {
                let time = self.duration_of(key);
                let time_percentage = time as f64 * 100.0 / total_time as f64;
                let global_percentage = time as f64 * 100.0 / global_time as f64;
                result.push(ResultField { name: key[path.len()..].to_string(), percentage: time_percentage, global_percentage, count: self.count_of(key) });
            }
        }
        if total_time as f32 > old_time {
            result.push(ResultField {
                name: "unspecified".to_string(),
                percentage: f64::from(total_time as f32 - old_time) * 100.0 / total_time as f64,
                global_percentage: f64::from(total_time as f32 - old_time) * 100.0 / global_time as f64,
                count: self_count,
            });
        }
        result.sort_by(|a, b| a.compare_to(b));
        result.insert(0, ResultField { name: raw_path.to_string(), percentage: 100.0, global_percentage: total_time as f64 * 100.0 / global_time as f64, count: self_count });
        result
    }

    fn get_counter_values(&self) -> BTreeMap<String, CounterCollector> {
        let mut result: BTreeMap<String, CounterCollector> = BTreeMap::new();
        for (path, entry) in &self.entries {
            if entry.counters.is_empty() {
                continue;
            }
            let segments: Vec<&str> = path.split(PATH_SEPARATOR).collect();
            for (counter, &value) in entry.counters.iter() {
                result.entry(counter.to_string()).or_default().add_value(&mut segments.iter(), value);
            }
        }
        result
    }

    pub fn save_results(&self, file: &Path) -> bool {
        let saved = (|| {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(file, self.get_profiler_results_for(self.get_nano_duration(), self.tick_duration))
        })();
        match saved {
            Ok(()) => true,
            Err(e) => {
                error!("Could not save profiler results to {}\n{e}", crate::util::java_nio::path_string(file));
                false
            }
        }
    }

    fn get_nano_duration(&self) -> i64 {
        self.end_time_nano - self.start_time_nano
    }

    fn get_profiler_results_for(&self, timespan: i64, tickspan: i32) -> String {
        let mut builder = String::new();
        ReportType::PROFILE.append_header(&mut builder, &[]);
        builder.push_str("Version: ");
        builder.push_str(wafer_data::VERSION_NAME);
        builder.push('\n');
        builder.push_str(&format!("Time span: {} ms\n", timespan / 1000000));
        builder.push_str(&format!("Tick span: {tickspan} ticks\n"));
        let ticks_per_second = tickspan as f32 / (timespan as f32 / 1.0e9f32);
        builder.push_str(&format!("// This is approximately {} ticks per second. It should be 20 ticks per second\n\n", java_format_fixed(f64::from(ticks_per_second), 2)));
        builder.push_str("--- BEGIN PROFILE DUMP ---\n\n");
        self.append_profiler_results(0, "root", &mut builder);
        builder.push_str("--- END PROFILE DUMP ---\n\n");
        let counters = self.get_counter_values();
        if !counters.is_empty() {
            builder.push_str("--- BEGIN COUNTER DUMP ---\n\n");
            self.append_counters(&counters, &mut builder, tickspan);
            builder.push_str("--- END COUNTER DUMP ---\n\n");
        }
        builder
    }

    fn append_profiler_results(&self, depth: i32, path: &str, builder: &mut String) {
        let results = self.get_times(path);
        if let Some(entry) = self.entries.get(path) {
            for (id, &value) in entry.counters.iter() {
                indent_line(builder, depth);
                builder.push_str(&format!("#{id} {value}/{}\n", java_div(value, self.tick_duration)));
            }
        }
        if results.len() >= 3 {
            for result in results.iter().skip(1) {
                indent_line(builder, depth);
                builder.push_str(&format!(
                    "{}({}/{}) - {}%/{}%\n",
                    result.name,
                    result.count,
                    java_format_fixed(f64::from(result.count as f32 / self.tick_duration as f32), 0),
                    java_format_fixed(result.percentage, 2),
                    java_format_fixed(result.global_percentage, 2)
                ));
                if result.name != "unspecified" {
                    self.append_profiler_results(depth + 1, &format!("{path}{PATH_SEPARATOR}{}", result.name), builder);
                }
            }
        }
    }

    fn append_counter_results(&self, depth: i32, name: &str, result: &CounterCollector, tickspan: i32, builder: &mut String) {
        indent_line(builder, depth);
        builder.push_str(&format!(
            "{name} total:{}/{} average: {}/{}\n",
            result.self_value,
            result.total_value,
            java_div(result.self_value, tickspan),
            java_div(result.total_value, tickspan)
        ));
        let mut children: Vec<(&str, &CounterCollector)> = result.children.iter().collect();
        children.sort_by(|a, b| b.1.total_value.cmp(&a.1.total_value));
        for (child_name, child) in children {
            self.append_counter_results(depth + 1, child_name, child, tickspan, builder);
        }
    }

    fn append_counters(&self, counters: &BTreeMap<String, CounterCollector>, builder: &mut String, tickspan: i32) {
        for (counter, counter_root) in counters {
            builder.push_str(&format!("-- Counter: {counter} --\n"));
            if let Some(root) = counter_root.children.get("root") {
                self.append_counter_results(0, "root", root, tickspan, builder);
            }
            builder.push_str("\n\n");
        }
    }
}

fn java_div(value: i64, divisor: i32) -> i64 {
    if divisor == 0 { 0 } else { value / i64::from(divisor) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(duration: i64, count: i64) -> PathEntry {
        PathEntry { accumulated_duration: duration, count, ..PathEntry::default() }
    }

    #[test]
    fn dump_lists_children_by_share_with_unspecified_time() {
        let mut entries = HashMap::new();
        entries.insert("root".to_string(), entry(1000, 10));
        entries.insert("root\u{1e}a".to_string(), entry(600, 10));
        entries.insert("root\u{1e}b".to_string(), entry(300, 10));
        let results = FilledProfileResults::new(entries, 0, 0, 1_000_000_000, 10);
        let text = results.get_profiler_results_for(results.get_nano_duration(), 10);
        assert!(text.contains("Version: "));
        assert!(text.contains("Time span: 1000 ms\nTick span: 10 ticks\n// This is approximately 10.00 ticks per second. It should be 20 ticks per second\n\n"));
        assert!(text.contains("--- BEGIN PROFILE DUMP ---\n\n[00] a(10/1) - 60.00%/60.00%\n[00] b(10/1) - 30.00%/30.00%\n[00] unspecified(10/1) - 10.00%/10.00%\n--- END PROFILE DUMP ---\n\n"));
        assert!(!text.contains("COUNTER DUMP"));
    }

    #[test]
    fn counters_are_summed_along_the_path() {
        let mut entries = HashMap::new();
        let mut collect = entry(10, 4);
        *collect.counters.entry_or_insert_with("containersToTick", || 0) += 8;
        entries.insert("root".to_string(), entry(100, 4));
        entries.insert("root\u{1e}tick".to_string(), entry(60, 4));
        entries.insert("root\u{1e}tick\u{1e}collect".to_string(), collect);
        let results = FilledProfileResults::new(entries, 0, 0, 1_000_000_000, 4);
        let text = results.get_profiler_results_for(results.get_nano_duration(), 4);
        assert!(text.contains("--- BEGIN COUNTER DUMP ---\n\n-- Counter: containersToTick --\n[00] root total:0/8 average: 0/2\n[01] |   tick total:0/8 average: 0/2\n[02] |   |   collect total:8/8 average: 2/2\n"));
    }

    #[test]
    fn equal_shares_sort_by_name_descending() {
        let mut entries = HashMap::new();
        entries.insert("root".to_string(), entry(200, 1));
        entries.insert("root\u{1e}a".to_string(), entry(100, 1));
        entries.insert("root\u{1e}b".to_string(), entry(100, 1));
        let results = FilledProfileResults::new(entries, 0, 0, 1_000_000_000, 1);
        let fields: Vec<String> = results.get_times("root").into_iter().map(|field| field.name).collect();
        assert_eq!(fields, ["root", "b", "a"]);
    }
}
