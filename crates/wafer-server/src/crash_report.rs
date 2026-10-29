use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

pub struct CrashReportCategory {
    title: String,
    entries: Vec<(String, String)>,
}

impl CrashReportCategory {
    pub fn format_location(min_y: i32, max_y: i32, x: i32, y: i32, z: i32) -> String {
        let (section_x, section_y, section_z) = (x >> 4, y >> 4, z >> 4);
        let (min_block_x, min_block_z) = (section_x << 4, section_z << 4);
        let (max_block_x, max_block_z) = (((section_x + 1) << 4) - 1, ((section_z + 1) << 4) - 1);
        let (region_x, region_z) = (x >> 9, z >> 9);
        format!(
            "World: ({x},{y},{z}), Section: (at {},{},{} in {section_x},{section_y},{section_z}; chunk contains blocks {min_block_x},{min_y},{min_block_z} to {max_block_x},{max_y},{max_block_z}), Region: ({region_x},{region_z}; contains chunks {},{} to {},{}, blocks {},{min_y},{} to {},{max_y},{})",
            x & 15,
            y & 15,
            z & 15,
            region_x << 5,
            region_z << 5,
            ((region_x + 1) << 5) - 1,
            ((region_z + 1) << 5) - 1,
            region_x << 9,
            region_z << 9,
            ((region_x + 1) << 9) - 1,
            ((region_z + 1) << 9) - 1,
        )
    }

    pub fn set_detail(&mut self, key: &str, value: impl Into<String>) -> &mut CrashReportCategory {
        self.entries.push((key.to_string(), value.into()));
        self
    }

    fn get_details(&self, builder: &mut String) {
        builder.push_str("-- ");
        builder.push_str(&self.title);
        builder.push_str(" --\n");
        builder.push_str("Details:");
        for (key, value) in &self.entries {
            builder.push_str("\n\t");
            builder.push_str(key);
            builder.push_str(": ");
            builder.push_str(value);
        }
    }
}

pub struct CrashReport {
    title: String,
    exception: String,
    stack_trace: Vec<String>,
    suppressed: Vec<String>,
    details: Vec<CrashReportCategory>,
    system_report: Vec<(String, String)>,
}

pub struct ReportedException(pub CrashReport);

static DELAYED_CRASH: std::sync::Mutex<Option<CrashReport>> = std::sync::Mutex::new(None);

pub fn relay_delay_crash(crash_report: CrashReport) {
    let mut delayed_crash = DELAYED_CRASH.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match delayed_crash.as_mut() {
        None => *delayed_crash = Some(crash_report),
        Some(delayed_crash) => delayed_crash.suppressed.push(crash_report.exception),
    }
}

pub fn throw_delayed_exception() {
    let delayed_crash = DELAYED_CRASH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    if let Some(delayed_crash) = delayed_crash {
        std::panic::panic_any(ReportedException(delayed_crash));
    }
}

pub fn exception_of(panic: &Box<dyn std::any::Any + Send>) -> String {
    if panic.is::<crate::world::level::biome::biome_source::ArithmeticException>() {
        return "java.lang.ArithmeticException: / by zero".to_string();
    }
    panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

impl CrashReport {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn exception(&self) -> &str {
        &self.exception
    }

    pub fn new(title: &str, exception: String, stack_trace: Vec<String>) -> CrashReport {
        CrashReport { title: title.to_string(), exception, stack_trace, suppressed: Vec::new(), details: Vec::new(), system_report: system_report() }
    }

    pub fn add_category(&mut self, name: &str) -> &mut CrashReportCategory {
        self.details.push(CrashReportCategory { title: name.to_string(), entries: Vec::new() });
        self.details.last_mut().unwrap()
    }

    pub fn set_system_detail(&mut self, key: &str, value: impl Into<String>) {
        self.system_report.push((key.to_string(), value.into()));
    }

    fn exception_message(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.exception);
        out.push_str(LINE_SEPARATOR);
        for frame in &self.stack_trace {
            out.push_str("\tat ");
            out.push_str(frame);
            out.push_str(LINE_SEPARATOR);
        }
        for suppressed in &self.suppressed {
            out.push_str("\tSuppressed: ");
            out.push_str(suppressed);
            out.push_str(LINE_SEPARATOR);
        }
        out
    }

    pub fn get_friendly_report(&self) -> String {
        let mut builder = String::new();
        builder.push_str("---- Minecraft Crash Report ----\n");
        builder.push_str("// ");
        builder.push_str(error_comment());
        builder.push_str("\n\n");
        builder.push_str("Time: ");
        builder.push_str(&Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        builder.push('\n');
        builder.push_str("Description: ");
        builder.push_str(&self.title);
        builder.push_str("\n\n");
        builder.push_str(&self.exception_message());
        builder.push_str("\n\nA detailed walkthrough of the error, its code path and all known details is as follows:\n");
        builder.push_str(&"-".repeat(87));
        builder.push_str("\n\n");
        for category in &self.details {
            category.get_details(&mut builder);
            builder.push_str("\n\n");
        }
        builder.push_str("-- System Details --\n");
        builder.push_str("Details:");
        for (key, value) in &self.system_report {
            builder.push_str("\n\t");
            builder.push_str(key);
            builder.push_str(": ");
            builder.push_str(value);
        }
        builder
    }

    pub fn save_to_file(&self, file: &Path) -> bool {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(file, self.get_friendly_report()) {
            Ok(()) => true,
            Err(_) => {
                error!("Could not save crash report to {}", file.display());
                false
            }
        }
    }
}

fn error_comment() -> &'static str {
    let nuggets = wafer_data::CRASH_REPORT_NUGGETS;
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    nuggets.get((nanos % nuggets.len().max(1) as u128) as usize).copied().unwrap_or("Witty comment unavailable :(")
}

pub fn crash_report_file(server_directory: &Path) -> std::path::PathBuf {
    server_directory.join("crash-reports").join(format!("crash-{}-server.txt", Local::now().format("%Y-%m-%d_%H.%M.%S")))
}

fn system_report() -> Vec<(String, String)> {
    vec![
        ("Minecraft Version".into(), wafer_data::VERSION_NAME.into()),
        ("Minecraft Version ID".into(), wafer_data::VERSION_NAME.into()),
        ("Operating System".into(), operating_system()),
        ("CPUs".into(), std::thread::available_parallelism().map_or(1, |n| n.get()).to_string()),
    ]
}

fn operating_system() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "x86" => "x86",
        "aarch64" => "aarch64",
        other => other,
    };
    let name = match std::env::consts::OS {
        "windows" => "Windows",
        "linux" => "Linux",
        "macos" => "Mac OS X",
        other => other,
    };
    format!("{name} ({arch})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_crashes_keep_the_first_report_and_suppress_later_ones() {
        relay_delay_crash(CrashReport::new("Exception chunk generation/loading", "java.util.concurrent.CompletionException: first".to_string(), Vec::new()));
        relay_delay_crash(CrashReport::new("Exception chunk generation/loading", "java.util.concurrent.CompletionException: second".to_string(), Vec::new()));
        let payload = crate::util::catch_exception(throw_delayed_exception).map(|_| ()).err();
        assert!(payload.is_some());
        let payload = std::panic::catch_unwind(|| {
            relay_delay_crash(CrashReport::new("Exception chunk generation/loading", "java.util.concurrent.CompletionException: third".to_string(), Vec::new()));
            throw_delayed_exception();
        })
        .unwrap_err();
        let reported = payload.downcast::<ReportedException>().ok().unwrap();
        assert_eq!(reported.0.title(), "Exception chunk generation/loading");
        assert_eq!(reported.0.exception_message(), format!("java.util.concurrent.CompletionException: third{LINE_SEPARATOR}"));
        let mut report = CrashReport::new("t", "java.lang.IllegalStateException: a".to_string(), vec!["f".to_string()]);
        report.suppressed.push("java.lang.RuntimeException: b".to_string());
        assert_eq!(report.exception_message(), format!("java.lang.IllegalStateException: a{LINE_SEPARATOR}\tat f{LINE_SEPARATOR}\tSuppressed: java.lang.RuntimeException: b{LINE_SEPARATOR}"));
        assert!(DELAYED_CRASH.lock().unwrap().is_none());
    }
}
