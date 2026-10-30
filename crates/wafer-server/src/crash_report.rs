use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;

use crate::system_report::SystemReport;
use crate::util::java_stack;
use crate::util::java_throwable::JavaThrowable;

pub const CRASH_REPORT_INIT_FRAME: &str = "net.minecraft.CrashReport.<init>(CrashReport.java:35)";
pub const FOR_THROWABLE_FRAME: &str = "net.minecraft.CrashReport.forThrowable(CrashReport.java:244)";
const PRELOAD_FRAME: &str = "net.minecraft.CrashReport.preload(CrashReport.java:252)";

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

pub struct ReportType {
    header: &'static str,
    nuggets: &'static [&'static str],
}

const CHUNK_IO_ERROR_NUGGETS: &[&str] = &[
    "I have failed you!",
    "Let's not do it again...",
    "Worst magic trick ever!",
    "Remember to backup your worlds regularly",
    "Pirates stole your chunk!",
    "Ker-chunk!",
    "Ideally, this shouldn't be here",
    "Let's hope it wasn't anything important",
    "Computers were a mistake",
    "Welp",
    "Not my proudest moment",
    "Who needs blocks in a block game, right?",
    "This chunk is no more...it has ceased to be...this is an EX-chunk",
    "loss.mca",
];

const PROFILE_NUGGETS: &[&str] = &[
    "I'd Rather Be Surfing",
    "Shiny numbers!",
    "Am I not running fast enough? :(",
    "I'm working as hard as I can!",
    "Will I ever be good enough for you? :(",
    "Speedy. Zoooooom!",
    "Hello world",
    "40% better than a crash report.",
    "Now with extra numbers",
    "Now with less numbers",
    "Now with the same numbers",
    "You should add flames to things, it makes them go faster!",
    "Do you feel the need for... optimization?",
    "*cracks redstone whip*",
    "Maybe if you treated it better then it'll have more motivation to work faster! Poor server.",
];

const TEST_NUGGETS: &[&str] = &[
    "Don't mind me",
    "One day I will be a real crash!",
    "Booo! Haha, did I scare you?",
    "Help, I'm trapped in a report factory!",
    "Have I answered your question?",
    "No hugs here, sorry",
    "I Can't Believe It's Not A Crash Report!",
    "Where's the kaboom!?",
];

impl ReportType {
    pub const CRASH: ReportType = ReportType { header: "Minecraft Crash Report", nuggets: wafer_data::CRASH_REPORT_NUGGETS };
    pub const PROFILE: ReportType = ReportType { header: "Minecraft Profiler Results", nuggets: PROFILE_NUGGETS };
    pub const TEST: ReportType = ReportType { header: "Minecraft Test Report", nuggets: TEST_NUGGETS };
    pub const CHUNK_IO_ERROR: ReportType = ReportType { header: "Minecraft Chunk IO Error Report", nuggets: CHUNK_IO_ERROR_NUGGETS };

    pub fn get_error_comment(&self) -> &'static str {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        self.nuggets.get((nanos % self.nuggets.len().max(1) as u128) as usize).copied().unwrap_or("Witty comment unavailable :(")
    }

    pub fn append_header(&self, builder: &mut String, extra_comments: &[String]) {
        builder.push_str("---- ");
        builder.push_str(self.header);
        builder.push_str(" ----\n");
        builder.push_str("// ");
        builder.push_str(self.get_error_comment());
        builder.push('\n');
        for extra_comment in extra_comments {
            builder.push_str("// ");
            builder.push_str(extra_comment);
            builder.push('\n');
        }
        builder.push('\n');
    }
}

#[derive(Clone)]
pub struct CrashReportCategory {
    title: String,
    entries: Vec<(String, String)>,
    stack_trace: Vec<String>,
}

fn frame_signature(frame: &str) -> (&str, &str) {
    let method = frame.split_once('(').map_or(frame, |(method, _)| method);
    let location = frame.split_once('(').and_then(|(_, rest)| rest.strip_suffix(')')).unwrap_or("");
    (method, location.split_once(':').map_or(location, |(file, _)| file))
}

impl CrashReportCategory {
    fn validate_stack_trace(&mut self, source: Option<&str>, next: Option<&str>) -> bool {
        let Some(source) = source.filter(|_| !self.stack_trace.is_empty()) else { return false };
        if frame_signature(&self.stack_trace[0]) != frame_signature(source) {
            return false;
        }
        if next.is_some() != (self.stack_trace.len() > 1) {
            return false;
        }
        if next.is_some_and(|next| self.stack_trace[1] != next) {
            return false;
        }
        self.stack_trace[0] = source.to_string();
        true
    }

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
        if !self.stack_trace.is_empty() {
            builder.push_str("\nStacktrace:");
            for element in &self.stack_trace {
                builder.push_str("\n\tat ");
                builder.push_str(element);
            }
        }
    }
}

#[derive(Clone)]
pub struct CrashReport {
    title: String,
    exception: String,
    stack_trace: Vec<String>,
    throwable: Option<JavaThrowable>,
    suppressed: Vec<String>,
    details: Vec<CrashReportCategory>,
    system_report: Vec<(String, String)>,
    tracking_stack_trace: bool,
    uncategorized_stack_trace: Vec<String>,
    save_file: Option<PathBuf>,
    reported_frames: Vec<&'static str>,
}

pub struct ReportedException(pub CrashReport);

pub struct JavaException(pub JavaThrowable);

pub fn throw_java_exception(throwable: JavaThrowable) -> ! {
    std::panic::panic_any(JavaException(throwable))
}

static DELAYED_CRASH: std::sync::Mutex<Option<CrashReport>> = std::sync::Mutex::new(None);

pub fn relay_delay_crash(crash_report: CrashReport) {
    let mut delayed_crash = DELAYED_CRASH.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match delayed_crash.as_mut() {
        None => *delayed_crash = Some(crash_report),
        Some(delayed_crash) => delayed_crash.suppressed.push(crash_report.exception),
    }
}

const THROW_DELAYED_EXCEPTION_FRAMES: [&str; 4] = [
    "net.minecraft.util.thread.BlockableEventLoop.throwDelayedException(BlockableEventLoop.java:203)",
    "net.minecraft.util.thread.BlockableEventLoop.pollTask(BlockableEventLoop.java:137)",
    "net.minecraft.server.MinecraftServer.pollTaskInternal(MinecraftServer.java:894)",
    "net.minecraft.server.MinecraftServer.pollTask(MinecraftServer.java:888)",
];

pub fn throw_delayed_exception() {
    let delayed_crash = DELAYED_CRASH.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    if let Some(delayed_crash) = delayed_crash {
        let mut frames = THROW_DELAYED_EXCEPTION_FRAMES.to_vec();
        frames.extend(crate::server::main_thread_frames::current());
        delayed_crash.throw_reported(frames);
    }
}

const ON_THREAD_EXCEPTION_CREATE_FRAME: &str = "net.minecraft.util.Util.onThreadException(Util.java:311)";

const ON_THREAD_EXCEPTION_FRAMES: [&str; 2] = [
    "net.minecraft.util.Util.onThreadException(Util.java:313)",
    "java.base/java.lang.Thread.dispatchUncaughtException(Thread.java:2556)",
];

pub fn on_thread_exception(thread_name: &str, thread_id: u32, throwable: &JavaThrowable) {
    error!(thread: thread_name, "Caught exception in thread Thread[#{thread_id},{thread_name},5,main]\n{}", throwable.print_stack_trace().trim_end());
    let mut report = {
        let _creator = java_stack::enter_all(&[ON_THREAD_EXCEPTION_CREATE_FRAME, ON_THREAD_EXCEPTION_FRAMES[1]]);
        CrashReport::for_throwable(throwable, "Exception on worker thread")
    };
    report.add_category_with_stack_trace("ThreadInfo", ON_THREAD_EXCEPTION_FRAMES.iter().map(|frame| frame.to_string()).collect()).set_detail("Name", thread_name);
    relay_delay_crash(report);
}

pub fn exception_of(panic: &Box<dyn std::any::Any + Send>) -> String {
    if panic.is::<crate::world::level::biome::biome_source::ArithmeticException>() {
        return "java.lang.ArithmeticException: / by zero".to_string();
    }
    if let Some(exception) = panic.downcast_ref::<JavaException>() {
        return exception.0.description.clone();
    }
    panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

pub fn throwable_of(panic: &Box<dyn std::any::Any + Send>) -> JavaThrowable {
    match panic.downcast_ref::<JavaException>() {
        Some(exception) => exception.0.clone(),
        None => JavaThrowable::from_panic(&exception_of(panic)),
    }
}

impl CrashReport {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn exception(&self) -> &str {
        &self.exception
    }

    pub fn new(title: &str, exception: String, stack_trace: Vec<String>) -> CrashReport {
        CrashReport {
            title: title.to_string(),
            exception,
            stack_trace,
            throwable: None,
            suppressed: Vec::new(),
            details: Vec::new(),
            system_report: {
                let _init = java_stack::enter(CRASH_REPORT_INIT_FRAME);
                SystemReport::new().into_entries()
            },
            tracking_stack_trace: true,
            uncategorized_stack_trace: Vec::new(),
            save_file: None,
            reported_frames: Vec::new(),
        }
    }

    pub fn for_throwable(throwable: &JavaThrowable, title: &str) -> CrashReport {
        let _scope = java_stack::enter(FOR_THROWABLE_FRAME);
        let mut throwable = throwable;
        while throwable.class_name() == "java.util.concurrent.CompletionException" {
            match throwable.cause.as_deref() {
                Some(cause) => throwable = cause,
                None => break,
            }
        }
        if let (Some(info), Some(cause)) = (&throwable.report, throwable.cause.as_deref()) {
            let mut report = CrashReport::of_exception(&info.title, cause);
            let category = report.add_category_with_stack_trace(&info.category, info.category_stack_trace.iter().map(|frame| frame.to_string()).collect());
            for (key, value) in &info.details {
                category.set_detail(key, value.clone());
            }
            return report;
        }
        CrashReport::of_exception(title, throwable)
    }

    pub fn of_exception(title: &str, exception: &JavaThrowable) -> CrashReport {
        let mut report = CrashReport::new(title, exception.description.clone(), exception.stack_trace.iter().map(|frame| frame.to_string()).collect());
        report.throwable = Some(exception.clone());
        report
    }

    pub fn get_save_file(&self) -> Option<&Path> {
        self.save_file.as_deref()
    }

    pub fn throw_reported(mut self, frames: Vec<&'static str>) -> ! {
        self.reported_frames = frames;
        std::panic::panic_any(ReportedException(self))
    }

    pub fn reported_exception(&self) -> JavaThrowable {
        let cause = match &self.throwable {
            Some(throwable) => throwable.clone(),
            None => JavaThrowable::new(self.exception.clone(), self.stack_trace.iter().map(|frame| &*Box::leak(frame.clone().into_boxed_str())).collect()),
        };
        JavaThrowable::new(format!("net.minecraft.ReportedException: {}", self.title), self.reported_frames.clone()).with_cause(cause)
    }


    pub fn add_category(&mut self, name: &str) -> &mut CrashReportCategory {
        self.details.push(CrashReportCategory { title: name.to_string(), entries: Vec::new(), stack_trace: Vec::new() });
        self.details.last_mut().unwrap()
    }

    pub fn add_category_with_stack_trace(&mut self, name: &str, stack_trace: Vec<String>) -> &mut CrashReportCategory {
        let mut category = CrashReportCategory { title: name.to_string(), entries: Vec::new(), stack_trace };
        if self.tracking_stack_trace {
            let size = category.stack_trace.len();
            let full_trace = &self.stack_trace;
            let trace_index = full_trace.len() as isize - size as isize;
            if trace_index < 0 {
                error!("Negative index in crash report handler ({}/{})", full_trace.len(), size);
            }
            let (mut source, mut next) = (None, None);
            if 0 <= trace_index && (trace_index as usize) < full_trace.len() {
                source = Some(full_trace[trace_index as usize].as_str());
                if full_trace.len() + 1 - size < full_trace.len() {
                    next = Some(full_trace[full_trace.len() + 1 - size].as_str());
                }
            }
            self.tracking_stack_trace = category.validate_stack_trace(source, next);
            if full_trace.len() >= size && 0 <= trace_index && (trace_index as usize) < full_trace.len() {
                self.uncategorized_stack_trace = full_trace[..trace_index as usize].to_vec();
            } else {
                self.tracking_stack_trace = false;
            }
        }
        self.details.push(category);
        self.details.last_mut().unwrap()
    }

    pub fn set_system_detail(&mut self, key: &str, value: impl Into<String>) {
        self.system_report.push((key.to_string(), value.into()));
    }

    fn exception_message(&self) -> String {
        if let Some(throwable) = &self.throwable {
            return throwable.print_stack_trace_with_separator(LINE_SEPARATOR);
        }
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

    pub fn get_friendly_report(&self, report_type: &ReportType) -> String {
        self.get_friendly_report_with_comments(report_type, &[])
    }

    pub fn get_friendly_report_with_comments(&self, report_type: &ReportType, extra_comments: &[String]) -> String {
        let mut builder = String::new();
        report_type.append_header(&mut builder, extra_comments);
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
        let uncategorized_stack_trace = match (self.uncategorized_stack_trace.is_empty(), self.details.first()) {
            (true, Some(category)) => category.stack_trace.iter().take(1).cloned().collect(),
            _ => self.uncategorized_stack_trace.clone(),
        };
        if !uncategorized_stack_trace.is_empty() {
            builder.push_str("-- Head --\n");
            builder.push_str("Thread: ");
            builder.push_str(&self.thread_name());
            builder.push('\n');
            builder.push_str("Stacktrace:\n");
            for element in &uncategorized_stack_trace {
                builder.push_str("\tat ");
                builder.push_str(element);
                builder.push('\n');
            }
            builder.push('\n');
        }
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

    fn thread_name(&self) -> String {
        std::thread::current().name().unwrap_or_default().to_string()
    }

    pub fn save_to_file(&mut self, save_file: &Path, report_type: &ReportType) -> bool {
        if self.save_file.is_some() {
            return false;
        }
        let saved = (|| {
            if let Some(parent) = save_file.parent().filter(|parent| !parent.as_os_str().is_empty()) {
                crate::util::file_util::create_directories_safe(parent)?;
            }
            std::fs::write(save_file, self.get_friendly_report(report_type))
        })();
        match saved {
            Ok(()) => {
                self.save_file = Some(save_file.to_path_buf());
                true
            }
            Err(_) => {
                error!("Could not save crash report to {}", save_file.display());
                false
            }
        }
    }
}

pub fn crash_report_file(server_directory: &Path) -> std::path::PathBuf {
    server_directory.join("crash-reports").join(format!("crash-{}-server.txt", crate::util::get_filename_formatted_date_time()))
}

pub fn preload() {
    let _scope = java_stack::enter_all(&[PRELOAD_FRAME]);
    CrashReport::new("Don't panic!", "java.lang.Throwable".to_string(), vec![PRELOAD_FRAME.to_string()]).get_friendly_report(&ReportType::CRASH);
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
        assert_eq!(
            reported.0.exception_message(),
            format!("java.util.concurrent.CompletionException: first{LINE_SEPARATOR}\tSuppressed: java.util.concurrent.CompletionException: second{LINE_SEPARATOR}\tSuppressed: java.util.concurrent.CompletionException: third{LINE_SEPARATOR}")
        );
        assert!(reported.0.reported_exception().print_stack_trace().starts_with("net.minecraft.ReportedException: Exception chunk generation/loading\n\tat net.minecraft.util.thread.BlockableEventLoop.throwDelayedException(BlockableEventLoop.java:203)\n"));
        *DELAYED_CRASH.lock().unwrap() = None;
        let mut report = CrashReport::new("t", "java.lang.IllegalStateException: a".to_string(), vec!["f".to_string()]);
        report.suppressed.push("java.lang.RuntimeException: b".to_string());
        assert_eq!(report.exception_message(), format!("java.lang.IllegalStateException: a{LINE_SEPARATOR}\tat f{LINE_SEPARATOR}\tSuppressed: java.lang.RuntimeException: b{LINE_SEPARATOR}"));
    }
}
